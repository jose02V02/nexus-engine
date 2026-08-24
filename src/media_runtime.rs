//! Media element playback control plane for Nexus Engine.
//!
//! This module owns container/codec discovery, decoder selection, packet and
//! decoded-frame queues, the media clock, buffering and A/V presentation. It
//! deliberately keeps platform decoding behind an adapter boundary.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::web_platform::{MediaCodec, MediaElementKind, MediaState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaContainer { Mp4, WebM, Ogg, AdtsAac, Unknown }

#[must_use]
pub fn sniff_container(bytes: &[u8]) -> MediaContainer {
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" { return MediaContainer::Mp4; }
    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) { return MediaContainer::WebM; }
    if bytes.starts_with(b"OggS") { return MediaContainer::Ogg; }
    if bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xf6 == 0xf0 { return MediaContainer::AdtsAac; }
    MediaContainer::Unknown
}

#[must_use]
pub fn parse_codec_string(value: &str) -> Option<MediaCodec> {
    let value = value.trim().to_ascii_lowercase();
    match value.split('.').next().unwrap_or("") {
        "av01" => Some(MediaCodec::Av1),
        "vp09" | "vp9" => Some(MediaCodec::Vp9),
        "avc1" | "avc3" | "h264" => Some(MediaCodec::H264),
        "opus" => Some(MediaCodec::Opus),
        "mp4a" | "aac" => Some(MediaCodec::Aac),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderBackend { AndroidMediaCodec, VideoToolbox, WgpuVideo, Software }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderCapability {
    pub codec: MediaCodec,
    pub backend: DecoderBackend,
    pub hardware_accelerated: bool,
    pub secure_decode: bool,
    pub max_width: u32,
    pub max_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderRequest {
    pub codec: MediaCodec,
    pub width: u32,
    pub height: u32,
    pub encrypted: bool,
    pub prefer_hardware: bool,
}

#[derive(Debug, Default)]
pub struct DecoderRegistry { capabilities: Vec<DecoderCapability> }

impl DecoderRegistry {
    pub fn register(&mut self, capability: DecoderCapability) {
        self.capabilities.push(capability);
    }

    #[must_use]
    pub fn select(&self, request: &DecoderRequest) -> Option<&DecoderCapability> {
        let mut candidates = self.capabilities.iter().filter(|candidate| {
            candidate.codec == request.codec
                && request.width <= candidate.max_width
                && request.height <= candidate.max_height
                && (!request.encrypted || candidate.secure_decode)
        }).collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            let preferred = candidate.hardware_accelerated == request.prefer_hardware;
            (!preferred, !candidate.hardware_accelerated)
        });
        candidates.into_iter().next()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaTrack {
    pub id: u32,
    pub kind: MediaElementKind,
    pub codec: MediaCodec,
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPacket {
    pub track_id: u32,
    pub pts: Duration,
    pub dts: Duration,
    pub duration: Duration,
    pub keyframe: bool,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat { Rgba8, Nv12, Yuv420p }

#[derive(Debug, Clone, PartialEq)]
pub enum DecodedMedia {
    Video { track_id: u32, pts: Duration, duration: Duration, width: u32, height: u32, format: PixelFormat, planes: Vec<Vec<u8>> },
    Audio { track_id: u32, pts: Duration, duration: Duration, sample_rate: u32, channels: u16, samples: Vec<f32> },
}

impl DecodedMedia {
    #[must_use]
    pub fn pts(&self) -> Duration {
        match self { Self::Video { pts, .. } | Self::Audio { pts, .. } => *pts }
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        match self { Self::Video { duration, .. } | Self::Audio { duration, .. } => *duration }
    }

    #[must_use]
    pub fn is_video(&self) -> bool { matches!(self, Self::Video { .. }) }
}

#[derive(Debug, Clone)]
pub struct MediaClock {
    media_offset: Duration,
    wall_origin: Duration,
    playback_rate: f64,
    running: bool,
}

impl Default for MediaClock {
    fn default() -> Self {
        Self { media_offset: Duration::ZERO, wall_origin: Duration::ZERO, playback_rate: 1.0, running: false }
    }
}

impl MediaClock {
    pub fn play(&mut self, wall_time: Duration) {
        if !self.running { self.wall_origin = wall_time; self.running = true; }
    }

    pub fn pause(&mut self, wall_time: Duration) {
        if self.running { self.media_offset = self.position(wall_time); self.running = false; }
    }

    pub fn seek(&mut self, media_time: Duration, wall_time: Duration) {
        self.media_offset = media_time;
        self.wall_origin = wall_time;
    }

    pub fn set_playback_rate(&mut self, rate: f64, wall_time: Duration) -> Result<(), MediaError> {
        if !rate.is_finite() || !(0.25..=4.0).contains(&rate) { return Err(MediaError::InvalidPlaybackRate); }
        self.media_offset = self.position(wall_time);
        self.wall_origin = wall_time;
        self.playback_rate = rate;
        Ok(())
    }

    #[must_use]
    pub fn position(&self, wall_time: Duration) -> Duration {
        if !self.running { return self.media_offset; }
        let elapsed = wall_time.saturating_sub(self.wall_origin).as_secs_f64() * self.playback_rate;
        self.media_offset.saturating_add(Duration::from_secs_f64(elapsed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    MetadataMissing,
    DuplicateTrack(u32),
    UnknownTrack(u32),
    UnsupportedCodec(MediaCodec),
    InvalidPlaybackRate,
    SeekOutOfRange,
    Decode(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackSnapshot {
    pub state: MediaState,
    pub duration: Duration,
    pub current_time: Duration,
    pub buffered_until: Duration,
    pub encoded_packets: usize,
    pub decoded_frames: usize,
    pub dropped_video_frames: u64,
}

pub struct MediaPipeline {
    kind: MediaElementKind,
    state: MediaState,
    duration: Duration,
    tracks: HashMap<u32, MediaTrack>,
    decoders: HashMap<u32, DecoderCapability>,
    encoded: VecDeque<EncodedPacket>,
    decoded: VecDeque<DecodedMedia>,
    clock: MediaClock,
    buffered_until: Duration,
    end_of_stream: bool,
    dropped_video_frames: u64,
}

impl MediaPipeline {
    #[must_use]
    pub fn new(kind: MediaElementKind) -> Self {
        Self { kind, state: MediaState::Empty, duration: Duration::ZERO, tracks: HashMap::new(), decoders: HashMap::new(), encoded: VecDeque::new(), decoded: VecDeque::new(), clock: MediaClock::default(), buffered_until: Duration::ZERO, end_of_stream: false, dropped_video_frames: 0 }
    }

    pub fn load_metadata(&mut self, tracks: Vec<MediaTrack>, registry: &DecoderRegistry, prefer_hardware: bool) -> Result<(), MediaError> {
        self.state = MediaState::Loading;
        self.tracks.clear(); self.decoders.clear(); self.duration = Duration::ZERO;
        for track in tracks {
            if self.tracks.contains_key(&track.id) { self.state = MediaState::Failed; return Err(MediaError::DuplicateTrack(track.id)); }
            let request = DecoderRequest { codec: track.codec, width: track.width, height: track.height, encrypted: false, prefer_hardware };
            let Some(decoder) = registry.select(&request).cloned() else {
                self.state = MediaState::Failed;
                return Err(MediaError::UnsupportedCodec(track.codec));
            };
            self.duration = self.duration.max(track.duration);
            self.decoders.insert(track.id, decoder);
            self.tracks.insert(track.id, track);
        }
        if self.tracks.is_empty() { self.state = MediaState::Failed; return Err(MediaError::MetadataMissing); }
        self.state = MediaState::Paused;
        Ok(())
    }

    pub fn enqueue_packet(&mut self, packet: EncodedPacket) -> Result<(), MediaError> {
        if !self.tracks.contains_key(&packet.track_id) { return Err(MediaError::UnknownTrack(packet.track_id)); }
        self.buffered_until = self.buffered_until.max(packet.pts.saturating_add(packet.duration));
        self.encoded.push_back(packet);
        Ok(())
    }

    pub fn take_packet_for_decode(&mut self) -> Option<(EncodedPacket, DecoderCapability)> {
        let packet = self.encoded.pop_front()?;
        let decoder = self.decoders.get(&packet.track_id)?.clone();
        Some((packet, decoder))
    }

    pub fn push_decoded(&mut self, frame: DecodedMedia) -> Result<(), MediaError> {
        let track_id = match &frame { DecodedMedia::Video { track_id, .. } | DecodedMedia::Audio { track_id, .. } => *track_id };
        if !self.tracks.contains_key(&track_id) { return Err(MediaError::UnknownTrack(track_id)); }
        let position = self.decoded.partition_point(|queued| queued.pts() <= frame.pts());
        self.decoded.insert(position, frame);
        Ok(())
    }

    pub fn play(&mut self, wall_time: Duration) -> Result<(), MediaError> {
        if self.tracks.is_empty() { return Err(MediaError::MetadataMissing); }
        self.clock.play(wall_time); self.state = MediaState::Playing; Ok(())
    }

    pub fn pause(&mut self, wall_time: Duration) {
        self.clock.pause(wall_time);
        if self.state != MediaState::Failed { self.state = MediaState::Paused; }
    }

    pub fn seek(&mut self, media_time: Duration, wall_time: Duration) -> Result<(), MediaError> {
        if media_time > self.duration { return Err(MediaError::SeekOutOfRange); }
        self.clock.seek(media_time, wall_time); self.encoded.clear(); self.decoded.clear(); self.end_of_stream = false;
        if self.state == MediaState::Ended { self.state = MediaState::Paused; }
        Ok(())
    }

    pub fn take_presentable(&mut self, wall_time: Duration) -> Vec<DecodedMedia> {
        let now = self.clock.position(wall_time).min(self.duration);
        let late_video = Duration::from_millis(80);
        let mut ready = Vec::new();
        while self.decoded.front().is_some_and(|frame| frame.pts() <= now) {
            let frame = self.decoded.pop_front().expect("front was checked");
            if frame.is_video() && frame.pts().saturating_add(frame.duration()).saturating_add(late_video) < now {
                self.dropped_video_frames = self.dropped_video_frames.saturating_add(1);
            } else { ready.push(frame); }
        }
        if self.end_of_stream && self.encoded.is_empty() && self.decoded.is_empty() && now >= self.duration {
            self.state = MediaState::Ended; self.clock.pause(wall_time);
        }
        ready
    }

    pub fn mark_end_of_stream(&mut self) { self.end_of_stream = true; }

    #[must_use]
    pub fn snapshot(&self, wall_time: Duration) -> PlaybackSnapshot {
        PlaybackSnapshot { state: self.state, duration: self.duration, current_time: self.clock.position(wall_time).min(self.duration), buffered_until: self.buffered_until, encoded_packets: self.encoded.len(), decoded_frames: self.decoded.len(), dropped_video_frames: self.dropped_video_frames }
    }

    #[must_use]
    pub fn kind(&self) -> MediaElementKind { self.kind }
}
