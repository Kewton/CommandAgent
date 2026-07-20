pub mod banner;
pub mod command_receipt;
pub mod editor;
pub mod footer;
pub mod input_queue;
pub mod interrupt;
pub mod markdown;
pub mod presentation;
pub mod repl;
pub mod repl_output;
pub mod slash;
pub mod spinner;
pub mod status;
pub mod status_bus;
pub mod terminal;
pub mod terminal_summary;
pub mod ux_demo;

use std::sync::{Arc, Mutex};

use crate::config::Config;

use self::footer::{Footer, FreezeGuard};
use self::input_queue::InputQueue;
use self::interrupt::{InterruptMonitor, PauseGuard};
use self::markdown::{TerminalMarkdownRenderer, TerminalMarkdownStream};
use self::spinner::Spinner;
use self::status::UiStatus;

pub trait InteractionUi {
    fn before_model_call(&self, label: &str) -> UiGuard;
    fn before_tool_call(&self, name: &str) -> UiGuard;
    fn publish_status(&self, status: UiStatus);
    fn render_command_receipt(&self, _receipt: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn interrupted(&self) -> bool;
    fn force_interrupted(&self) -> bool {
        false
    }
}

pub trait OutputRenderer {
    fn render_assistant(&self, raw_text: &str) -> anyhow::Result<()>;
}

pub struct UiGuard {
    _footer: Option<FreezeGuard>,
    _spinner: Option<Spinner>,
    stream: Option<TerminalMarkdownStream>,
}

impl UiGuard {
    pub fn noop() -> Self {
        Self {
            _footer: None,
            _spinner: None,
            stream: None,
        }
    }

    fn active(
        footer: Option<FreezeGuard>,
        spinner: Option<Spinner>,
        stream: Option<TerminalMarkdownStream>,
    ) -> Self {
        Self {
            _footer: footer,
            _spinner: spinner,
            stream,
        }
    }

    pub fn push_assistant_chunk(&mut self, chunk: &str) -> anyhow::Result<()> {
        if chunk.is_empty() {
            return Ok(());
        }
        self._spinner.take();
        if let Some(stream) = self.stream.as_mut() {
            stream.push_chunk(chunk)?;
        }
        Ok(())
    }

    pub fn finish_assistant_stream(&mut self) -> anyhow::Result<()> {
        if let Some(stream) = self.stream.as_mut() {
            stream.finish()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NoopUi;

pub static NOOP_UI: NoopUi = NoopUi;

impl InteractionUi for NoopUi {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn interrupted(&self) -> bool {
        false
    }
}

pub struct TerminalUi {
    interrupt: Arc<Mutex<InterruptMonitor>>,
    footer: Footer,
    input_queue: InputQueue,
    stream: bool,
}

impl TerminalUi {
    pub fn new(config: &Config) -> Self {
        Self::new_inner(config, false)
    }

    pub fn new_with_input_queue(config: &Config) -> Self {
        Self::new_inner(config, true)
    }

    fn new_inner(config: &Config, queue_input: bool) -> Self {
        let input_queue = InputQueue::new();
        let footer = if queue_input {
            Footer::start_with_input_queue(config, input_queue.clone())
        } else {
            Footer::start(config)
        };
        let monitor = if queue_input {
            InterruptMonitor::start_with_input_queue(input_queue.clone())
        } else {
            InterruptMonitor::start()
        };
        let interrupt = Arc::new(Mutex::new(monitor));
        let input_enabled = queue_input
            && footer.is_active()
            && interrupt
                .lock()
                .map(|monitor| monitor.is_active())
                .unwrap_or(false);
        input_queue.set_enabled(input_enabled);
        let ui = Self {
            interrupt,
            footer,
            input_queue,
            stream: config.streaming_enabled(),
        };
        ui.publish_status(UiStatus::from_config(config));
        ui
    }

    pub fn pause_for_prompt(&self) -> PromptGuard {
        PromptGuard {
            _interrupt: PauseGuard::new(self.interrupt.clone()),
            _footer: self.footer.freeze(),
        }
    }

    pub fn reset_interrupt(&self) {
        if let Ok(monitor) = self.interrupt.lock() {
            monitor.reset();
        }
    }

    pub fn take_queued_input(&self) -> Option<String> {
        self.input_queue.take_queued()
    }

    pub fn take_pending_input(&self) -> Option<String> {
        self.input_queue.take_pending()
    }
}

impl InteractionUi for TerminalUi {
    fn before_model_call(&self, label: &str) -> UiGuard {
        let stream = self
            .stream
            .then(|| TerminalMarkdownRenderer::for_stdout().begin_stream());
        UiGuard::active(None, Spinner::start(label), stream)
    }

    fn before_tool_call(&self, name: &str) -> UiGuard {
        UiGuard::active(None, Spinner::start(format!("tool {name}")), None)
    }

    fn publish_status(&self, status: UiStatus) {
        self.footer.publish(status);
    }

    fn render_command_receipt(&self, receipt: &str) -> anyhow::Result<()> {
        self.footer.write_scrollback(receipt)
    }

    fn interrupted(&self) -> bool {
        self.interrupt
            .lock()
            .map(|monitor| monitor.interrupted())
            .unwrap_or(false)
    }

    fn force_interrupted(&self) -> bool {
        self.interrupt
            .lock()
            .map(|monitor| monitor.force_interrupted())
            .unwrap_or(false)
    }
}

pub struct PromptGuard {
    _interrupt: Option<PauseGuard>,
    _footer: Option<FreezeGuard>,
}
