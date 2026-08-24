//! WebRTC and MediaStream control plane.
//!
//! SDP/ICE validation, peer-connection state, media tracks and data-channel
//! queues are executable here. Socket ICE, DTLS-SRTP and device capture remain
//! platform adapters and are not simulated as secure transport.

use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpType { Offer, Answer }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind { Audio, Video, Application }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDirection { SendRecv, SendOnly, RecvOnly, Inactive }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpCodec {
    pub payload_type: u8,
    pub name: String,
    pub clock_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdpMediaSection {
    pub mid: String,
    pub kind: MediaKind,
    pub port: u16,
    pub protocol: String,
    pub payload_types: Vec<u8>,
    pub direction: MediaDirection,
    pub ice_ufrag: String,
    pub ice_pwd: String,
    pub fingerprint: String,
    pub codecs: Vec<RtpCodec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDescription {
    pub kind: SdpType,
    pub bundle_mids: Vec<String>,
    pub media: Vec<SdpMediaSection>,
}

impl SessionDescription {
    pub fn parse(kind: SdpType, source: &str) -> Result<Self, RtcError> {
        if !source.lines().any(|line| line.trim() == "v=0") { return Err(RtcError::InvalidSdp("missing v=0".to_owned())); }
        let mut bundle_mids = Vec::new();
        let mut media = Vec::<SdpMediaSection>::new();
        let mut session_ufrag = String::new();
        let mut session_pwd = String::new();
        let mut session_fingerprint = String::new();
        for raw in source.lines() {
            let line = raw.trim();
            if let Some(value) = line.strip_prefix("a=group:BUNDLE ") {
                bundle_mids = value.split_ascii_whitespace().map(str::to_owned).collect();
            } else if let Some(value) = line.strip_prefix("m=") {
                let fields = value.split_ascii_whitespace().collect::<Vec<_>>();
                if fields.len() < 4 { return Err(RtcError::InvalidSdp("invalid media line".to_owned())); }
                let kind = match fields[0] { "audio" => MediaKind::Audio, "video" => MediaKind::Video, "application" => MediaKind::Application, _ => return Err(RtcError::InvalidSdp("unsupported media kind".to_owned())) };
                let port = fields[1].parse().map_err(|_| RtcError::InvalidSdp("invalid media port".to_owned()))?;
                let payload_types = fields[3..].iter().filter_map(|value| value.parse().ok()).collect();
                media.push(SdpMediaSection { mid: String::new(), kind, port, protocol: fields[2].to_owned(), payload_types, direction: MediaDirection::SendRecv, ice_ufrag: session_ufrag.clone(), ice_pwd: session_pwd.clone(), fingerprint: session_fingerprint.clone(), codecs: Vec::new() });
            } else if let Some(value) = line.strip_prefix("a=ice-ufrag:") {
                if let Some(section) = media.last_mut() { section.ice_ufrag = value.to_owned(); } else { session_ufrag = value.to_owned(); }
            } else if let Some(value) = line.strip_prefix("a=ice-pwd:") {
                if let Some(section) = media.last_mut() { section.ice_pwd = value.to_owned(); } else { session_pwd = value.to_owned(); }
            } else if let Some(value) = line.strip_prefix("a=fingerprint:") {
                if let Some(section) = media.last_mut() { section.fingerprint = value.to_owned(); } else { session_fingerprint = value.to_owned(); }
            } else if let Some(value) = line.strip_prefix("a=mid:") {
                if let Some(section) = media.last_mut() { section.mid = value.to_owned(); }
            } else if line == "a=sendonly" { if let Some(section) = media.last_mut() { section.direction = MediaDirection::SendOnly; } }
            else if line == "a=recvonly" { if let Some(section) = media.last_mut() { section.direction = MediaDirection::RecvOnly; } }
            else if line == "a=inactive" { if let Some(section) = media.last_mut() { section.direction = MediaDirection::Inactive; } }
            else if let Some(value) = line.strip_prefix("a=rtpmap:") {
                let mut fields = value.split_ascii_whitespace();
                let payload_type = fields.next().and_then(|value| value.parse::<u8>().ok()).ok_or_else(|| RtcError::InvalidSdp("invalid rtpmap payload".to_owned()))?;
                let encoding = fields.next().ok_or_else(|| RtcError::InvalidSdp("missing rtpmap encoding".to_owned()))?;
                let parts = encoding.split('/').collect::<Vec<_>>();
                if parts.len() < 2 { return Err(RtcError::InvalidSdp("invalid rtpmap encoding".to_owned())); }
                let clock_rate = parts[1].parse().map_err(|_| RtcError::InvalidSdp("invalid RTP clock".to_owned()))?;
                let channels = parts.get(2).and_then(|value| value.parse().ok()).unwrap_or(1);
                if let Some(section) = media.last_mut() { section.codecs.push(RtpCodec { payload_type, name: parts[0].to_ascii_lowercase(), clock_rate, channels }); }
            }
        }
        if media.is_empty() { return Err(RtcError::InvalidSdp("description has no media sections".to_owned())); }
        for (index, section) in media.iter_mut().enumerate() {
            if section.mid.is_empty() { section.mid = index.to_string(); }
            if section.ice_ufrag.is_empty() || section.ice_pwd.is_empty() || section.fingerprint.is_empty() {
                return Err(RtcError::InvalidSdp("ICE credentials or DTLS fingerprint missing".to_owned()));
            }
        }
        Ok(Self { kind, bundle_mids, media })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceTransport { Udp, Tcp }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceCandidateType { Host, ServerReflexive, PeerReflexive, Relay }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidate {
    pub foundation: String,
    pub component: u16,
    pub transport: IceTransport,
    pub priority: u32,
    pub address: String,
    pub port: u16,
    pub candidate_type: IceCandidateType,
}

impl IceCandidate {
    pub fn parse(source: &str) -> Result<Self, RtcError> {
        let source = source.trim().strip_prefix("candidate:").unwrap_or(source.trim());
        let fields = source.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 8 || fields[6] != "typ" { return Err(RtcError::InvalidCandidate); }
        let transport = match fields[2].to_ascii_lowercase().as_str() { "udp" => IceTransport::Udp, "tcp" => IceTransport::Tcp, _ => return Err(RtcError::InvalidCandidate) };
        let candidate_type = match fields[7] { "host" => IceCandidateType::Host, "srflx" => IceCandidateType::ServerReflexive, "prflx" => IceCandidateType::PeerReflexive, "relay" => IceCandidateType::Relay, _ => return Err(RtcError::InvalidCandidate) };
        Ok(Self { foundation: fields[0].to_owned(), component: fields[1].parse().map_err(|_| RtcError::InvalidCandidate)?, transport, priority: fields[3].parse().map_err(|_| RtcError::InvalidCandidate)?, address: fields[4].to_owned(), port: fields[5].parse().map_err(|_| RtcError::InvalidCandidate)?, candidate_type })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePair { pub local: IceCandidate, pub remote: IceCandidate, pub priority: u64 }

fn best_candidate_pair(local: &[IceCandidate], remote: &[IceCandidate]) -> Option<CandidatePair> {
    local.iter().flat_map(|left| remote.iter().filter_map(move |right| {
        if left.component != right.component || left.transport != right.transport { return None; }
        let min = u64::from(left.priority.min(right.priority));
        let max = u64::from(left.priority.max(right.priority));
        let tie_break = if left.priority > right.priority { 1 } else { 0 };
        Some(CandidatePair { local: left.clone(), remote: right.clone(), priority: (min << 32).saturating_add(max.saturating_mul(2)).saturating_add(tie_break) })
    })).max_by_key(|pair| pair.priority)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState { Live, Ended }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrackConstraints { pub width: Option<u32>, pub height: Option<u32>, pub frame_rate: Option<u16>, pub sample_rate: Option<u32>, pub echo_cancellation: Option<bool> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaStreamTrack { pub id: String, pub kind: MediaKind, pub label: String, pub enabled: bool, pub muted: bool, pub state: TrackState, pub constraints: MediaTrackConstraints }

impl MediaStreamTrack {
    pub fn stop(&mut self) { self.state = TrackState::Ended; self.enabled = false; }
    pub fn set_muted(&mut self, muted: bool) { if self.state == TrackState::Live { self.muted = muted; } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaStream { pub id: String, tracks: Vec<MediaStreamTrack> }

impl MediaStream {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self { Self { id: id.into(), tracks: Vec::new() } }
    pub fn add_track(&mut self, track: MediaStreamTrack) -> Result<(), RtcError> {
        if self.tracks.iter().any(|candidate| candidate.id == track.id) { return Err(RtcError::DuplicateTrack); }
        self.tracks.push(track); Ok(())
    }
    pub fn remove_track(&mut self, id: &str) -> Option<MediaStreamTrack> { self.tracks.iter().position(|track| track.id == id).map(|index| self.tracks.remove(index)) }
    #[must_use]
    pub fn tracks(&self) -> &[MediaStreamTrack] { &self.tracks }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingState { Stable, HaveLocalOffer, HaveRemoteOffer, Closed }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceConnectionState { New, Checking, Connected, Failed, Closed }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataChannelState { Connecting, Open, Closing, Closed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMessage { Text(String), Binary(Vec<u8>) }

#[derive(Debug)]
pub struct DataChannel {
    pub label: String,
    pub ordered: bool,
    pub state: DataChannelState,
    max_buffered_bytes: usize,
    buffered_bytes: usize,
    outgoing: VecDeque<DataMessage>,
}

impl DataChannel {
    #[must_use]
    pub fn new(label: impl Into<String>, ordered: bool, max_buffered_bytes: usize) -> Self { Self { label: label.into(), ordered, state: DataChannelState::Connecting, max_buffered_bytes, buffered_bytes: 0, outgoing: VecDeque::new() } }
    pub fn open(&mut self) { if self.state == DataChannelState::Connecting { self.state = DataChannelState::Open; } }
    pub fn send(&mut self, message: DataMessage) -> Result<(), RtcError> {
        if self.state != DataChannelState::Open { return Err(RtcError::DataChannelNotOpen); }
        let bytes = match &message { DataMessage::Text(value) => value.len(), DataMessage::Binary(value) => value.len() };
        if self.buffered_bytes.saturating_add(bytes) > self.max_buffered_bytes { return Err(RtcError::DataChannelBackpressure); }
        self.buffered_bytes += bytes; self.outgoing.push_back(message); Ok(())
    }
    pub fn take_outgoing(&mut self) -> Option<DataMessage> {
        let message = self.outgoing.pop_front()?;
        self.buffered_bytes = self.buffered_bytes.saturating_sub(match &message { DataMessage::Text(value) => value.len(), DataMessage::Binary(value) => value.len() });
        Some(message)
    }
    #[must_use]
    pub fn buffered_bytes(&self) -> usize { self.buffered_bytes }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcError { InvalidSdp(String), InvalidCandidate, InvalidSignalingState, DuplicateTrack, DataChannelNotOpen, DataChannelBackpressure, Closed }

pub struct PeerConnection {
    signaling: SignalingState,
    ice: IceConnectionState,
    local_description: Option<SessionDescription>,
    remote_description: Option<SessionDescription>,
    local_candidates: Vec<IceCandidate>,
    remote_candidates: Vec<IceCandidate>,
    selected_pair: Option<CandidatePair>,
    streams: HashMap<String, MediaStream>,
    data_channels: Vec<DataChannel>,
}

impl Default for PeerConnection {
    fn default() -> Self { Self::new() }
}

impl PeerConnection {
    #[must_use]
    pub fn new() -> Self { Self { signaling: SignalingState::Stable, ice: IceConnectionState::New, local_description: None, remote_description: None, local_candidates: Vec::new(), remote_candidates: Vec::new(), selected_pair: None, streams: HashMap::new(), data_channels: Vec::new() } }

    pub fn set_local_description(&mut self, description: SessionDescription) -> Result<(), RtcError> {
        self.ensure_open()?;
        self.signaling = match (self.signaling, description.kind) { (SignalingState::Stable, SdpType::Offer) => SignalingState::HaveLocalOffer, (SignalingState::HaveRemoteOffer, SdpType::Answer) => SignalingState::Stable, _ => return Err(RtcError::InvalidSignalingState) };
        self.local_description = Some(description); self.update_ice(); Ok(())
    }

    pub fn set_remote_description(&mut self, description: SessionDescription) -> Result<(), RtcError> {
        self.ensure_open()?;
        self.signaling = match (self.signaling, description.kind) { (SignalingState::Stable, SdpType::Offer) => SignalingState::HaveRemoteOffer, (SignalingState::HaveLocalOffer, SdpType::Answer) => SignalingState::Stable, _ => return Err(RtcError::InvalidSignalingState) };
        self.remote_description = Some(description); self.update_ice(); Ok(())
    }

    pub fn add_local_candidate(&mut self, candidate: IceCandidate) -> Result<(), RtcError> { self.ensure_open()?; self.local_candidates.push(candidate); self.update_ice(); Ok(()) }
    pub fn add_remote_candidate(&mut self, candidate: IceCandidate) -> Result<(), RtcError> { self.ensure_open()?; self.remote_candidates.push(candidate); self.update_ice(); Ok(()) }
    pub fn add_stream(&mut self, stream: MediaStream) -> Result<(), RtcError> { self.ensure_open()?; if self.streams.contains_key(&stream.id) { return Err(RtcError::DuplicateTrack); } self.streams.insert(stream.id.clone(), stream); Ok(()) }
    pub fn create_data_channel(&mut self, label: impl Into<String>, ordered: bool, max_buffered_bytes: usize) -> Result<usize, RtcError> { self.ensure_open()?; self.data_channels.push(DataChannel::new(label, ordered, max_buffered_bytes)); Ok(self.data_channels.len() - 1) }
    pub fn data_channel_mut(&mut self, index: usize) -> Option<&mut DataChannel> { self.data_channels.get_mut(index) }

    pub fn close(&mut self) { self.signaling = SignalingState::Closed; self.ice = IceConnectionState::Closed; for channel in &mut self.data_channels { channel.state = DataChannelState::Closed; } }

    #[must_use] pub fn signaling_state(&self) -> SignalingState { self.signaling }
    #[must_use] pub fn ice_state(&self) -> IceConnectionState { self.ice }
    #[must_use] pub fn selected_pair(&self) -> Option<&CandidatePair> { self.selected_pair.as_ref() }

    fn update_ice(&mut self) {
        if self.local_description.is_some() && self.remote_description.is_some() { self.ice = IceConnectionState::Checking; }
        self.selected_pair = best_candidate_pair(&self.local_candidates, &self.remote_candidates);
        if self.selected_pair.is_some() && self.local_description.is_some() && self.remote_description.is_some() { self.ice = IceConnectionState::Connected; }
    }

    fn ensure_open(&self) -> Result<(), RtcError> { if self.signaling == SignalingState::Closed { Err(RtcError::Closed) } else { Ok(()) } }
}
