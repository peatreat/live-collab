use shared::*;

use crossbeam::queue::SegQueue;
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Color32, CornerRadius, Vec2},
    resizable_window::ResizableWindow,
    EguiState,
};
use tokio::{io::AsyncReadExt, runtime::Runtime};
use webrtc::{data_channel::data_channel_message::DataChannelMessage, peer_connection::peer_connection_state::RTCPeerConnectionState};
use std::{io::Cursor, sync::{Arc, LazyLock, Mutex}};

static TEXT_VALUE_ENTRY_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 3)));
static PAGE_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 4)));
static WEBRTC_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 5)));

pub struct Receiver {
    params: Arc<ReceiverParams>,
}

#[derive(Params)]
pub struct ReceiverParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    pub page: IntParam,
    
    pub runtime: Runtime,
    pub messages: Arc<SegQueue<f32>>,
}

impl Default for Receiver {
    fn default() -> Self {
        Self {
            params: Arc::new(ReceiverParams::default()),
        }
    }
}

impl Default for ReceiverParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(300, 180),

            page: IntParam::new("page", 0, IntRange::Linear { min: 0, max: 1 }),
            messages: Default::default(),
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl Plugin for Receiver {
    const NAME: &'static str = "Live Collab Receiver (mono)";
    const VENDOR: &'static str = "peatreat";
    const URL: &'static str = "https://github.com/peatreat/live-collab";
    const EMAIL: &'static str = "";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = params.editor_state.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, _, _state| {
                ResizableWindow::new("Live Collab Receiver")
                    .min_size(Vec2::new(128.0, 128.0))
                    .show(egui_ctx, egui_state.as_ref(), |ui| {
                        egui_ctx.all_styles_mut(|style| {
                            style.visuals.panel_fill = Color32::from_rgb(7, 17, 38); // white bg
                            style.spacing.indent = 16.0;
                            style.spacing.item_spacing = Vec2::new(16.0, 16.0);
                            style.visuals.window_corner_radius = CornerRadius::ZERO;
                            style.visuals.extreme_bg_color = Color32::from_rgb(29, 31, 36);
                        });

                        let connection: Option<WebRTCConnection> = ui.memory(|mem| { mem.data.get_temp(*WEBRTC_MEMORY_ID) });

                        if let Some(connection) = &connection {
                            ui.label(format!("Connection State: {}", connection.peer.connection_state().to_string()));

                            if ui.button("Disconnect").clicked() {
                                let conn_clone = connection.clone();
                                params.runtime.spawn(async move { conn_clone.peer.close().await });
                                ui.memory_mut(|mem| mem.data.insert_temp(*PAGE_MEMORY_ID, 0));
                            }
                        }

                        match ui.memory(|mem| { mem.data.get_temp(*PAGE_MEMORY_ID).unwrap_or(0) }) {
                            0 => {
                                let value_entry_mutex = ui.memory_mut(|mem| {
                                    mem.data
                                        .get_temp_mut_or_default::<Arc<Mutex<String>>>(*TEXT_VALUE_ENTRY_MEMORY_ID)
                                        .clone()
                                });
                                let mut value_entry = value_entry_mutex.lock().unwrap();

                                let text_input_label = ui.label("Enter peer offer:");
                                ui.text_edit_singleline(&mut *value_entry).labelled_by(text_input_label.id);

                                if ui.button("Connect").clicked() {
                                    if let Ok(connection) = create_answerer(&params.runtime, value_entry.to_owned()) {
                                        let params_clone = params.clone();

                                        *value_entry = Default::default();

                                        connection.channel.on_message(Box::new(move |msg: DataChannelMessage| {
                                            let p2 = params_clone.clone();
                                            Box::pin(async move {
                                                let mut rdr = Cursor::new(msg.data);
                                                while let Ok(val) = rdr.read_f32_le().await {
                                                    p2.messages.push(val);
                                                }
                                            })
                                        }));

                                        let conn_clone = connection.clone();
                                        connection.tcp_channel.on_message(Box::new(move |msg| {
                                            let cc2 = conn_clone.clone();
                                            Box::pin(async move {
                                                let _ = cc2.tcp_channel.send(&bytes::Bytes::copy_from_slice(&msg.data)).await;
                                            })
                                        }));

                                        ui.memory_mut(|mem| mem.data.insert_temp(*WEBRTC_MEMORY_ID, connection));
                                        ui.memory_mut(|mem| mem.data.insert_temp(*PAGE_MEMORY_ID, 1));
                                    }
                                }
                            },
                            1 => {
                                if ui.button("Go back").clicked() {
                                    ui.memory_mut(|mem| mem.data.insert_temp(*PAGE_MEMORY_ID, 0));
                                }

                                if let Some(connection) = &connection {
                                    let send_label = ui.label("Send this to peer:");
                                    if ui.button("Copy Session Token").labelled_by(send_label.id).clicked() {
                                        ui.ctx().copy_text(connection.connect_info.to_owned());
                                    }

                                    ui.label("Samples Buffered: ".to_owned() + &params.messages.len().to_string());

                                    if ui.button("Clear Buffered Samples").clicked() {
                                        while !params.messages.is_empty() {
                                            if params.messages.pop().is_none() {
                                                break;
                                            }
                                        }
                                    }

                                    if connection.peer.connection_state() == RTCPeerConnectionState::Failed {
                                        ui.memory_mut(|mem| mem.data.insert_temp(*PAGE_MEMORY_ID, 0));
                                    }
                                }
                            },
                            _ => {}
                        }
                    });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();

        unsafe { buffer.set_slices(num_samples, |output| {
            for i in 0..num_samples {
                if let Some(recv_sample) = self.params.messages.pop() {
                    output[0][i] = recv_sample;
                    output[1][i] = recv_sample;
                } else {
                    output[0][i] = 0.0;
                    output[1][i] = 0.0;
                }
            }
        }) };

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Receiver {
    const CLAP_ID: &'static str = "com.moist-plugins-gmbh-egui.live-collab-receiver-gui";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("WebRTC Audio Receiver");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Receiver {
    const VST3_CLASS_ID: [u8; 16] = *b"LiveCollabRecvvv";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Receiver);
nih_export_vst3!(Receiver);
