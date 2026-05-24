use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use directories::ProjectDirs;
use run_loop_timer::Timer;
use wrac_clap_adapter::{
    GuiConfiguration, GuiSize, HostGuiResizeRequester, HostParameterEditNotifier, PluginError,
    PluginResult,
};
use wrac_wxp_gui::{DpiConverter, ParentWindowHandle, WxpGuiResizeHandle, WxpGuiRuntime};
use wxp::{WebContext, WxpCommandHandler, WxpWebView, WxpWebViewBuilder, dpi::LogicalSize};

use crate::commands::register_commands;
use crate::gui::GuiStateNotifier;
use crate::plugin::{
    PARAM_BYPASS_ID, PARAM_DELAY_FEEDBACK_ID, PARAM_DELAY_MIX_ID, PARAM_DELAY_TIME_ID,
    PARAM_GAIN_ID, PARAM_PITCH_ID, PARAM_PORTAMENTO_ID, PARAM_VOWEL_ID, PLUGIN_ID,
};
use crate::state::{ProjectStateStore, SharedState};

// GUI window size. Echo Lama's pane is locked to the 396 × 573 layout (66% of the
// original 600 × 868 design, matching the 912 × 1320 ratio of `static/bg.png`); the
// frontend's CSS scales the 600 × 868 design down to fit. The host is told the window is
// not resizable by clamping min == max.
pub(super) const DEFAULT_GUI_SIZE: GuiSize = GuiSize {
    width: 396,
    height: 573,
};
pub(super) const MIN_GUI_SIZE: GuiSize = GuiSize {
    width: 396,
    height: 573,
};
pub(super) const MAX_GUI_SIZE: GuiSize = GuiSize {
    width: 396,
    height: 573,
};

// Embed the frontend zip only for release builds; debug builds use the Vite dev server.
#[cfg(not(debug_assertions))]
const FRONTEND_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/echo_lama_gui.zip"));

#[derive(Clone)]
pub(super) struct GuiRuntimeDependencies {
    pub(super) project_state: Arc<ProjectStateStore>,
    pub(super) shared: Arc<SharedState>,
    pub(super) gui_notifier: Arc<GuiStateNotifier>,
    pub(super) host_parameter_edit_notifier: Arc<dyn HostParameterEditNotifier>,
    pub(super) host_gui_resize_requester: Arc<dyn HostGuiResizeRequester>,
    pub(super) resize_handle: WxpGuiResizeHandle,
}

/// Runtime for one GUI window. Created each time the host opens the GUI; dropped when closed.
pub(crate) struct EchoLamaGuiRuntime {
    gui_notifier: Arc<GuiStateNotifier>,
    // !Send + !Sync token owning the native WebView. Stored as Option to control drop order.
    web_view: Option<WxpWebView>,
    // Kept alive longer than the WebView (see the Drop impl below for ordering).
    wxp_context: Option<WebContext>,
    command_handler: Rc<WxpCommandHandler>,
    // Timer that periodically pushes the current shared state to the GUI.
    gui_update_timer: Timer,
    gui_size: LogicalSize<f64>,
    // Used for bounds conversion that accounts for DPI scaling.
    dpi_converter: DpiConverter,
}

impl EchoLamaGuiRuntime {
    /// Factory called from the closure in `plugin.rs` when the host requests the GUI to open.
    /// Creates a WebView attached to the parent window and returns it.
    pub(super) fn create(
        dependencies: GuiRuntimeDependencies,
        configuration: GuiConfiguration,
        initial_size: GuiSize,
        parent: ParentWindowHandle,
    ) -> PluginResult<Self> {
        // This sample supports only embedded mode (attached to the parent).
        // Implement floating window support separately if needed.
        if configuration.is_floating {
            log::warn!("rejecting floating GUI configuration");
            return Err(PluginError::Message("unsupported GUI configuration"));
        }
        log::debug!(
            "creating GUI runtime: width={}, height={}, configuration={configuration:?}",
            initial_size.width,
            initial_size.height
        );

        // Register parameter commands callable from the WebView.
        log::debug!("creating GUI runtime: creating command handler");
        let command_handler = Rc::new(WxpCommandHandler::new());
        log::debug!("creating GUI runtime: registering commands");
        register_commands(
            command_handler.clone(),
            dependencies.project_state.clone(),
            dependencies.shared.clone(),
            dependencies.gui_notifier.clone(),
            dependencies.host_parameter_edit_notifier,
            dependencies.host_gui_resize_requester,
            dependencies.resize_handle,
        );
        log::debug!("creating GUI runtime: commands registered");

        // WebView2 can fail to initialise when two instances share the same user data folder
        // with different Environment options, so isolate each plugin by ID under the
        // OS standard app-data directory.
        let data_dir = webview_data_dir(PLUGIN_ID);
        std::fs::create_dir_all(&data_dir)
            .map_err(|_| PluginError::Message("failed to create GUI data directory"))?;
        log::debug!("using GUI data directory: {}", data_dir.display());

        log::debug!("creating GUI runtime: creating WebContext");
        let mut wxp_context = WebContext::new(data_dir);
        // Initial scale is 1.0; the host will override it via `set_scale` later.
        let dpi_converter = DpiConverter::new(1.0);
        let gui_size = dpi_converter.gui_size_to_logical(initial_size);
        let bounds = dpi_converter.create_webview_bounds(gui_size);
        log::debug!(
            "creating GUI runtime: computed logical size: width={}, height={}",
            gui_size.width,
            gui_size.height
        );

        // Debug builds point to the Vite dev server (no native rebuild needed on frontend changes).
        // Release builds cannot depend on a dev server, so the zip bundled by build.rs is served.
        #[cfg(debug_assertions)]
        let builder = {
            let url = "http://127.0.0.1:5173/";
            log::debug!("creating GUI runtime: configuring debug WebView builder: url={url}");
            WxpWebViewBuilder::new(&mut wxp_context)
                .with_command_handler(command_handler.clone())
                .with_devtools(cfg!(debug_assertions))
                .with_visible(true)
                .with_bounds(bounds)
                .with_url(url)
        };

        #[cfg(not(debug_assertions))]
        let builder = {
            let url = "wxp-plugin://localhost/";
            log::debug!("creating GUI runtime: configuring release WebView builder: url={url}");
            WxpWebViewBuilder::new(&mut wxp_context)
                .with_command_handler(command_handler.clone())
                .with_devtools(cfg!(debug_assertions))
                .with_visible(true)
                .with_bounds(bounds)
                // Serve the embedded zip under the `wxp-plugin://` scheme.
                .with_serve_zip("wxp-plugin", FRONTEND_ZIP)
                .map_err(|_| PluginError::Message("failed to serve GUI assets"))?
                .with_url(url)
        };

        // Create the WebView as a child of the parent window, embedding it in the host UI.
        log::debug!("creating GUI runtime: build_as_child start");
        let web_view = builder
            .build_as_child(&parent)
            .map_err(|_| PluginError::Message("failed to build webview"))?;
        log::debug!("creating GUI runtime: build_as_child completed");

        // Push the current value to the GUI at ~30 Hz (33 ms). Reading shared state on
        // every tick is simpler than maintaining a dirty flag. CLAP's `request_callback()`
        // depends on the host's dispatch implementation when going through a wrapper and
        // can leave the GUI with stale values, so a timer on the GUI runtime's own run
        // loop is used instead.
        //
        // Voicing is also pushed every tick so the character's mouth open/closed state
        // tracks the audio thread's `is_voicing` atomic — the audio thread does not push
        // notifications directly because it has no GUI notifier handle.
        let gui_update_timer = Timer::new(Duration::from_millis(33), {
            let shared = dependencies.shared.clone();
            let gui_notifier = dependencies.gui_notifier.clone();
            move || {
                gui_notifier.notify_parameter(PARAM_GAIN_ID, shared.gain());
                gui_notifier.notify_parameter(PARAM_PITCH_ID, shared.pitch());
                gui_notifier.notify_parameter(PARAM_VOWEL_ID, shared.vowel());
                gui_notifier.notify_parameter(PARAM_PORTAMENTO_ID, shared.portamento());
                gui_notifier.notify_parameter(PARAM_DELAY_TIME_ID, shared.delay_time());
                gui_notifier.notify_parameter(PARAM_DELAY_MIX_ID, shared.delay_mix());
                gui_notifier.notify_parameter(PARAM_DELAY_FEEDBACK_ID, shared.delay_feedback());
                gui_notifier.notify_parameter(PARAM_BYPASS_ID, f32::from(shared.bypass()));
                gui_notifier.notify_voicing(shared.is_voicing());
            }
        });
        log::debug!("creating GUI runtime: starting GUI update timer");
        gui_update_timer.start();
        log::debug!("creating GUI runtime: GUI update timer started");

        log::debug!("creating GUI runtime: completed");
        Ok(Self {
            gui_notifier: dependencies.gui_notifier,
            web_view: Some(web_view),
            wxp_context: Some(wxp_context),
            command_handler,
            gui_update_timer,
            gui_size,
            dpi_converter,
        })
    }
}

// Trait implementation for resize, scale, and size operations called by the host.
impl WxpGuiRuntime for EchoLamaGuiRuntime {
    /// Called when the host reports a display scale factor (e.g. HiDPI).
    fn set_scale(&mut self, scale: f64) -> PluginResult<()> {
        log::debug!("setting GUI scale: scale={scale}");
        self.dpi_converter.set_scale(scale);
        Ok(())
    }

    /// Called when the host changes the window size. Clamps to the valid range before applying.
    fn set_size(&mut self, size: GuiSize) -> PluginResult<()> {
        let clamped = GuiSize {
            width: size.width.clamp(MIN_GUI_SIZE.width, MAX_GUI_SIZE.width),
            height: size.height.clamp(MIN_GUI_SIZE.height, MAX_GUI_SIZE.height),
        };
        self.gui_size = self.dpi_converter.gui_size_to_logical(clamped);
        log::debug!(
            "setting GUI size: requested_width={}, requested_height={}, applied_width={}, applied_height={}",
            size.width,
            size.height,
            self.gui_size.width,
            self.gui_size.height
        );

        if let Some(web_view) = &self.web_view {
            // wxp separates direct native WebView manipulation from the owner. Even though this
            // is already on the GUI thread, dispatch is used to align with stale-close checks
            // and the post/enqueue semantics used elsewhere.
            web_view
                .dispatch()
                .post_set_bounds(self.dpi_converter.create_webview_bounds(self.gui_size))
                .map_err(|_| PluginError::Message("failed to resize webview"))?;
        }
        Ok(())
    }

    fn show(&mut self) -> PluginResult<()> {
        log::debug!("showing GUI runtime");
        if let Some(web_view) = &self.web_view {
            // show/hide often races with host lifecycle events, so the close-aware dispatch
            // path in wxp is used rather than touching the owner directly.
            web_view
                .dispatch()
                .post_set_visible(true)
                .map_err(|_| PluginError::Message("failed to show webview"))?;
        }
        self.gui_update_timer.start();
        log::debug!("showing GUI runtime completed");
        Ok(())
    }

    fn hide(&mut self) -> PluginResult<()> {
        log::debug!("hiding GUI runtime");
        self.gui_update_timer.stop();
        if let Some(web_view) = &self.web_view {
            // hide can be called just before destroy. If the WebView is already closed,
            // dispatch returns WebViewClosed without extending the native object's lifetime.
            web_view
                .dispatch()
                .post_set_visible(false)
                .map_err(|_| PluginError::Message("failed to hide webview"))?;
        }
        log::debug!("hiding GUI runtime completed");
        Ok(())
    }
}

fn webview_data_dir(plugin_id: &str) -> PathBuf {
    let plugin_dir = sanitize_plugin_data_dir(plugin_id);
    // Derive the WebView user-data path from plugin_id too. Hard-coding the template name
    // here would cause a renamed plugin to share cookies, cache, and storage with the original.
    match project_dirs_from_plugin_id(plugin_id) {
        Some(dirs) => dirs.data_dir().join("webview").join(plugin_dir),
        None => std::env::temp_dir()
            .join(plugin_dir)
            .join("webview")
            .join("data"),
    }
}

fn project_dirs_from_plugin_id(plugin_id: &str) -> Option<ProjectDirs> {
    let mut parts = plugin_id.split('.');
    let qualifier = parts.next()?;
    let organization = parts.next()?;
    let application = parts.collect::<Vec<_>>().join("-");
    if application.is_empty() {
        return None;
    }
    ProjectDirs::from(qualifier, organization, &application)
}

fn sanitize_plugin_data_dir(plugin_id: &str) -> String {
    plugin_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

// Override the field-declaration drop order and explicitly sequence:
// disconnect → destroy WebView → destroy context.
// This prevents callbacks from touching already-freed objects.
impl Drop for EchoLamaGuiRuntime {
    fn drop(&mut self) {
        log::debug!("dropping GUI runtime");
        // The timer callback depends on the run loop and GUI subscriptions. Stop it before
        // dropping the native WebView so no tick can observe partially-destroyed GUI state.
        self.gui_update_timer.stop();
        log::debug!("dropping GUI runtime: timer stopped");
        // The GUI is going away; clear channels from shared state as well.
        self.gui_notifier.clear_subscriptions();
        log::debug!("dropping GUI runtime: subscriptions cleared");
        // Drop WebView before WebContext. The reverse order can cause wry to panic
        // when the context is absent during WebView teardown.
        self.web_view = None;
        log::debug!("dropping GUI runtime: webview dropped");
        self.wxp_context = None;
        log::debug!("dropping GUI runtime: web context dropped");
        // `command_handler` and `gui_update_timer` are left to field drop order.
        // The two reads below make the intended "keep alive until here" explicit.
        let _ = Rc::strong_count(&self.command_handler);
        let _ = self.gui_update_timer.is_running();
    }
}
