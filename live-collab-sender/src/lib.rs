use shared::*;

use bytes::{Buf, Bytes};
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Color32, CornerRadius, Vec2, Window},
    resizable_window::ResizableWindow,
    EguiState,
};
use tokio::runtime::Runtime;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use std::{sync::{Arc, LazyLock, Mutex}, time::{SystemTime, UNIX_EPOCH}};

static PAGE_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 4)));
static WEBRTC_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 5)));
static ANSWER_VALUE_ENTRY_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 6)));
static ERROR_VALUE_ENTRY_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 7)));

pub struct Sender {
    params: Arc<SenderParams>,
}

#[derive(Params)]
pub struct SenderParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
    
    pub buffer_size: IntParam,

    pub page: IntParam,

    pub round_trip_latency: AtomicF32,
    pub sample_buffer: Arc<crossbeam::queue::SegQueue<f32>>,
    pub runtime: Runtime,
    pub connection: Arc<Mutex<Option<WebRTCConnection>>>,
}

impl Default for Sender {
    fn default() -> Self {
        Self {
            params: Arc::new(SenderParams::default()),
        }
    }
}

impl Default for SenderParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(300, 180),

            buffer_size: IntParam::new("buffer-size", 64, IntRange::Linear { min: 0, max: 2048 }),
            page: IntParam::new("page", 0, IntRange::Linear { min: 0, max: 1 }),
            connection: Default::default(),
            runtime: Runtime::new().unwrap(),
            sample_buffer: Default::default(),
            round_trip_latency: Default::default(),
        }
    }
}

impl Plugin for Sender {
    const NAME: &'static str = "Live Collab Sender (mono)";
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
                ResizableWindow::new("Live Collab Sender")
                    .min_size(Vec2::new(300.0, 300.0))
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
                            
                            ui.label(format!("Round-Trip Latency ({} ms)", params.round_trip_latency.load(std::sync::atomic::Ordering::Relaxed)));

                            if connection.peer.connection_state() == RTCPeerConnectionState::Connected {
                                if ui.button("Disconnect").clicked() {
                                    let conn_clone = connection.clone();
                                    params.runtime.spawn(async move { conn_clone.peer.close().await });
                                    ui.memory_mut(|mem| mem.data.insert_temp(*PAGE_MEMORY_ID, 0));
                                }
                            }
                        }

                        match ui.memory(|mem| { mem.data.get_temp(*PAGE_MEMORY_ID).unwrap_or(0) }) {
                            0 => {
                                if ui.button("Create Session").clicked() {
                                    if let Ok(connection) = create_offerer(&params.runtime) {
                                        *params.connection.lock().unwrap() = Some(connection.clone());

                                        let params_clone = params.clone();
                                        let conn_clone = connection.clone();

                                        connection.tcp_channel.on_message(Box::new(move |mut msg| {
                                            let cur_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

                                            if msg.data.len() == 16 {
                                                let recv_ts = msg.data.get_u128_le();
                                                
                                                params_clone.round_trip_latency.store((cur_ts - recv_ts) as f32, std::sync::atomic::Ordering::Relaxed);
                                            }

                                            let cc2 = conn_clone.clone();
                                            Box::pin(async move {
                                                let _ = cc2.tcp_channel.send(&Bytes::copy_from_slice(&cur_ts.to_le_bytes())).await;
                                            })
                                        }));

                                        let conn_clone = connection.clone();
                                        params.runtime.spawn(async move {
                                            let cur_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                                            conn_clone.channel.send(&Bytes::copy_from_slice(&cur_ts.to_le_bytes())).await
                                        });
                                        
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

                                    let value_entry_mutex = ui.memory_mut(|mem| {
                                        mem.data
                                            .get_temp_mut_or_default::<Arc<Mutex<String>>>(*ANSWER_VALUE_ENTRY_MEMORY_ID)
                                            .clone()
                                    });

                                    let mut value_entry = value_entry_mutex.lock().unwrap();

                                    let text_input_label = ui.label("Enter peer answer:");
                                    ui.text_edit_singleline(&mut *value_entry).labelled_by(text_input_label.id);

                                    if ui.button("Set Answer").clicked() {
                                        let error_value_entry_mutex = ui.memory_mut(|mem| {
                                            mem.data
                                                .get_temp_mut_or_default::<Arc<Mutex<String>>>(*ERROR_VALUE_ENTRY_MEMORY_ID)
                                                .clone()
                                        });
                                        
                                        let mut error_value_entry = error_value_entry_mutex.lock().unwrap();

                                        if let Err(err) = connection.set_answer(&params.runtime, value_entry.to_owned()) {
                                            *error_value_entry =  err.to_string();
                                        }
                                    }

                                    {
                                        let error_value_entry_mutex = ui.memory_mut(|mem| {
                                            mem.data
                                                .get_temp_mut_or_default::<Arc<Mutex<String>>>(*ERROR_VALUE_ENTRY_MEMORY_ID)
                                                .clone()
                                        });
                                        
                                        let mut error_value_entry = error_value_entry_mutex.lock().unwrap();

                                        ui.label(error_value_entry.to_owned());
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
        if let Some(connection) = &*self.params.connection.lock().unwrap() {
            if connection.peer.connection_state() == RTCPeerConnectionState::Connected {
                let num_samples = buffer.samples();
                let mut samples = Vec::with_capacity(num_samples);

                unsafe { 
                    buffer.set_slices(num_samples, |output| {
                        for i in 0..num_samples {
                            samples.push((output[0][i] + output[1][i]) / 2.0);
                        }
                    })
                };

                if !samples.is_empty() {
                    let conn_clone = connection.clone();

                    self.params.runtime.spawn(async move {
                        let samples_as_bytes = samples.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<_>>();
                        conn_clone.channel.send(&Bytes::copy_from_slice(samples_as_bytes.as_slice())).await
                    });
                }
            }   
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Sender {
    const CLAP_ID: &'static str = "com.moist-plugins-gmbh-egui.live-collab-sender-gui";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("WebRTC Audio Sender");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Sender {
    const VST3_CLASS_ID: [u8; 16] = *b"LiveCollabSender";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Sender);
nih_export_vst3!(Sender);
