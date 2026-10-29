//! Channel connection parameters owned by the Host settings file.
//!
//! Get, set, connect, and disconnect are a fold over that catalog. A rejected
//! credential does not replace a saved channel. Connect and disconnect record
//! status in the same catalog. This module does not call a channel network.

use serde::{Deserialize, Serialize};

const MAX_CHANNELS: usize = 16;
const INVALID: &str = "Invalid channel settings";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Wechat,
    Wecom,
    Feishu,
    Dingtalk,
    Slack,
    Teams,
    Discord,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelReason {
    CredentialRejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelRecovery {
    ReplaceCredential,
    EditParameters,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelFailureReason {
    CredentialRejected,
    InvalidParameters,
    UnknownChannel,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ChannelView {
    pub id: String,
    pub kind: ChannelKind,
    pub endpoint: String,
    pub account: String,
    pub credential_configured: bool,
    pub status: ChannelStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<ChannelReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<ChannelRecovery>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChannelCatalogView {
    pub channels: Vec<ChannelView>,
}

/// Rejection returned to the client. It carries no credential bytes.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ChannelFailure {
    pub id: String,
    pub status: ChannelStatus,
    pub reason: ChannelFailureReason,
    pub recovery: ChannelRecovery,
}

impl ChannelFailure {
    pub fn invalid(id: &str) -> Self {
        Self {
            id: shown_id(id),
            status: ChannelStatus::Error,
            reason: ChannelFailureReason::InvalidParameters,
            recovery: ChannelRecovery::EditParameters,
        }
    }

    fn credential(id: &str) -> Self {
        Self {
            id: shown_id(id),
            status: ChannelStatus::Error,
            reason: ChannelFailureReason::CredentialRejected,
            recovery: ChannelRecovery::ReplaceCredential,
        }
    }

    fn unknown(id: &str) -> Self {
        Self {
            id: shown_id(id),
            status: ChannelStatus::Error,
            reason: ChannelFailureReason::UnknownChannel,
            recovery: ChannelRecovery::EditParameters,
        }
    }

    pub fn message_key(&self) -> &'static str {
        match self.reason {
            ChannelFailureReason::CredentialRejected => "error.channel.credential",
            ChannelFailureReason::InvalidParameters | ChannelFailureReason::UnknownChannel => {
                "error.channel"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelChange {
    Keep(ChannelCatalog),
    Store(ChannelCatalog),
}

impl ChannelChange {
    pub fn catalog(&self) -> &ChannelCatalog {
        match self {
            Self::Keep(catalog) | Self::Store(catalog) => catalog,
        }
    }

    pub fn persists(&self) -> bool {
        matches!(self, Self::Store(_))
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct StoredChannel {
    id: String,
    kind: ChannelKind,
    endpoint: String,
    account: String,
    credential: String,
    status: ChannelStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<ChannelReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovery: Option<ChannelRecovery>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ChannelCatalog {
    channels: Vec<StoredChannel>,
}

impl std::fmt::Debug for ChannelCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChannelCatalog")
            .field("count", &self.channels.len())
            .field(
                "ids",
                &self
                    .channels
                    .iter()
                    .map(|channel| channel.id.as_str())
                    .collect::<Vec<_>>(),
            )
            .field(
                "status",
                &self
                    .channels
                    .iter()
                    .map(|channel| channel.status)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ChannelCatalog {
    pub fn view(&self) -> ChannelCatalogView {
        ChannelCatalogView {
            channels: self
                .channels
                .iter()
                .map(|channel| ChannelView {
                    id: channel.id.clone(),
                    kind: channel.kind,
                    endpoint: channel.endpoint.clone(),
                    account: channel.account.clone(),
                    credential_configured: true,
                    status: channel.status,
                    reason: match channel.status {
                        ChannelStatus::Error => channel.reason,
                        ChannelStatus::Connected | ChannelStatus::Disconnected => None,
                    },
                    recovery: match channel.status {
                        ChannelStatus::Error => channel.recovery,
                        ChannelStatus::Connected | ChannelStatus::Disconnected => None,
                    },
                })
                .collect(),
        }
    }

    pub fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.channels).expect("stored channels serialize")
    }

    pub fn from_file_value(value: &serde_json::Value) -> Result<Self, &'static str> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let channels: Vec<StoredChannel> =
            serde_json::from_value(value.clone()).map_err(|_| INVALID)?;
        if channels.len() > MAX_CHANNELS {
            return Err(INVALID);
        }
        for (index, channel) in channels.iter().enumerate() {
            if !stored_ok(channel) || channels[..index].iter().any(|other| other.id == channel.id) {
                return Err(INVALID);
            }
        }
        Ok(Self { channels })
    }
}

#[derive(Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum ChannelSettingsRequest {
    #[serde(rename = "settings.channel.get")]
    Get,
    #[serde(rename = "settings.channel.set")]
    Set {
        id: String,
        kind: ChannelKind,
        endpoint: String,
        account: String,
        credential: String,
    },
    #[serde(rename = "settings.channel.connect")]
    Connect { id: String },
    #[serde(rename = "settings.channel.disconnect")]
    Disconnect { id: String },
}

impl ChannelSettingsRequest {
    pub fn get() -> Self {
        Self::Get
    }

    pub fn set(
        id: impl Into<String>,
        kind: ChannelKind,
        endpoint: impl Into<String>,
        account: impl Into<String>,
        credential: impl Into<String>,
    ) -> Self {
        Self::Set {
            id: id.into(),
            kind,
            endpoint: endpoint.into(),
            account: account.into(),
            credential: credential.into(),
        }
    }

    pub fn connect(id: impl Into<String>) -> Self {
        Self::Connect { id: id.into() }
    }

    pub fn disconnect(id: impl Into<String>) -> Self {
        Self::Disconnect { id: id.into() }
    }

    /// `(catalog, request) -> next catalog`. Reads and writes nothing.
    pub fn apply(&self, catalog: &ChannelCatalog) -> Result<ChannelChange, ChannelFailure> {
        match self {
            Self::Get => Ok(ChannelChange::Keep(catalog.clone())),
            Self::Set {
                id,
                kind,
                endpoint,
                account,
                credential,
            } => set_channel(catalog, id, *kind, endpoint, account, credential),
            Self::Connect { id } => connect_channel(catalog, id),
            Self::Disconnect { id } => disconnect_channel(catalog, id),
        }
    }
}

impl std::fmt::Debug for ChannelSettingsRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => formatter.write_str("Get"),
            Self::Set {
                id,
                kind,
                endpoint,
                account,
                credential,
            } => formatter
                .debug_struct("Set")
                .field("id", id)
                .field("kind", kind)
                .field("endpoint", endpoint)
                .field("account", account)
                .field("credential_present", &!credential.is_empty())
                .finish(),
            Self::Connect { id } => formatter.debug_struct("Connect").field("id", id).finish(),
            Self::Disconnect { id } => formatter
                .debug_struct("Disconnect")
                .field("id", id)
                .finish(),
        }
    }
}

fn credential_ok(secret: &str) -> bool {
    credential_accepted(secret).is_some()
}

fn set_channel(
    catalog: &ChannelCatalog,
    id: &str,
    kind: ChannelKind,
    endpoint: &str,
    account: &str,
    credential: &str,
) -> Result<ChannelChange, ChannelFailure> {
    if !id_ok(id) || !endpoint_ok(endpoint) || !account_ok(account) {
        return Err(ChannelFailure::invalid(id));
    }
    if !credential_ok(credential) {
        return Err(ChannelFailure::credential(id));
    }
    if let Some(existing) = catalog.channels.iter().find(|channel| channel.id == id) {
        if existing.kind == kind
            && existing.endpoint == endpoint
            && existing.account == account
            && existing.credential == credential
        {
            return Ok(ChannelChange::Keep(catalog.clone()));
        }
    } else if catalog.channels.len() == MAX_CHANNELS {
        return Err(ChannelFailure::invalid(id));
    }
    let record = StoredChannel {
        id: id.to_owned(),
        kind,
        endpoint: endpoint.to_owned(),
        account: account.to_owned(),
        credential: credential.to_owned(),
        status: ChannelStatus::Disconnected,
        reason: None,
        recovery: None,
    };
    let mut channels = catalog.channels.clone();
    if let Some(slot) = channels.iter_mut().find(|channel| channel.id == id) {
        *slot = record;
    } else {
        channels.push(record);
    }
    Ok(ChannelChange::Store(ChannelCatalog { channels }))
}

fn connect_channel(catalog: &ChannelCatalog, id: &str) -> Result<ChannelChange, ChannelFailure> {
    if !id_ok(id) {
        return Err(ChannelFailure::invalid(id));
    }
    let Some(index) = catalog.channels.iter().position(|channel| channel.id == id) else {
        return Err(ChannelFailure::unknown(id));
    };
    let mut next = catalog.clone();
    let channel = &mut next.channels[index];
    if !credential_ok(&channel.credential) {
        if channel.status == ChannelStatus::Error
            && channel.reason == Some(ChannelReason::CredentialRejected)
            && channel.recovery == Some(ChannelRecovery::ReplaceCredential)
        {
            return Ok(ChannelChange::Keep(catalog.clone()));
        }
        channel.status = ChannelStatus::Error;
        channel.reason = Some(ChannelReason::CredentialRejected);
        channel.recovery = Some(ChannelRecovery::ReplaceCredential);
        return Ok(ChannelChange::Store(next));
    }
    if channel.status == ChannelStatus::Connected {
        return Ok(ChannelChange::Keep(catalog.clone()));
    }
    channel.status = ChannelStatus::Connected;
    channel.reason = None;
    channel.recovery = None;
    Ok(ChannelChange::Store(next))
}

fn disconnect_channel(catalog: &ChannelCatalog, id: &str) -> Result<ChannelChange, ChannelFailure> {
    if !id_ok(id) {
        return Err(ChannelFailure::invalid(id));
    }
    let Some(index) = catalog.channels.iter().position(|channel| channel.id == id) else {
        return Err(ChannelFailure::unknown(id));
    };
    if catalog.channels[index].status == ChannelStatus::Disconnected
        && catalog.channels[index].reason.is_none()
        && catalog.channels[index].recovery.is_none()
    {
        return Ok(ChannelChange::Keep(catalog.clone()));
    }
    let mut next = catalog.clone();
    next.channels[index].status = ChannelStatus::Disconnected;
    next.channels[index].reason = None;
    next.channels[index].recovery = None;
    Ok(ChannelChange::Store(next))
}

fn stored_ok(channel: &StoredChannel) -> bool {
    id_ok(&channel.id)
        && endpoint_ok(&channel.endpoint)
        && account_ok(&channel.account)
        && decode_envelope(&channel.credential).is_some()
        && match channel.status {
            ChannelStatus::Connected | ChannelStatus::Disconnected => {
                channel.reason.is_none() && channel.recovery.is_none()
            }
            ChannelStatus::Error => {
                channel.reason == Some(ChannelReason::CredentialRejected)
                    && channel.recovery == Some(ChannelRecovery::ReplaceCredential)
            }
        }
}

/// Accepted envelope bytes, or `None` when the credential must be replaced.
fn credential_accepted(secret: &str) -> Option<Vec<u8>> {
    let bytes = decode_envelope(secret)?;
    (!bytes.starts_with(b"reject:")).then_some(bytes)
}

fn id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphanumeric())
        && id.len() <= 64
        && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '-'))
}

fn account_ok(value: &str) -> bool {
    let count = value.chars().count();
    (1..=128).contains(&count) && value == value.trim() && !value.chars().any(char::is_control)
}

fn endpoint_ok(value: &str) -> bool {
    (12..=2048).contains(&value.len())
        && value == value.trim()
        && value.starts_with("https://")
        && !value
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
        && !value.contains('@')
        && value
            .as_bytes()
            .get(8)
            .is_some_and(u8::is_ascii_alphanumeric)
        && value[8..].contains('.')
}

fn shown_id(id: &str) -> String {
    id.chars().take(128).collect()
}

fn decode_envelope(secret: &str) -> Option<Vec<u8>> {
    let bytes = secret.as_bytes();
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if !(4..=4096).contains(&bytes.len())
        || bytes.len() % 4 != 0
        || padding > 2
        || bytes.len() == padding
    {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut index = 0;
    while index < bytes.len() {
        let sextet = |byte: u8| match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        };
        if bytes[index] == b'=' || bytes[index + 1] == b'=' {
            return None;
        }
        let padded = |position: usize| bytes[position] == b'=';
        if padded(index + 2) && (!padded(index + 3) || index + 4 != bytes.len()) {
            return None;
        }
        if padded(index + 3) && index + 4 != bytes.len() {
            return None;
        }
        let chunk = [
            sextet(bytes[index])?,
            sextet(bytes[index + 1])?,
            if padded(index + 2) {
                0
            } else {
                sextet(bytes[index + 2])?
            },
            if padded(index + 3) {
                0
            } else {
                sextet(bytes[index + 3])?
            },
        ];
        let value = (u32::from(chunk[0]) << 18)
            | (u32::from(chunk[1]) << 12)
            | (u32::from(chunk[2]) << 6)
            | u32::from(chunk[3]);
        out.push((value >> 16) as u8);
        if !padded(index + 2) {
            out.push((value >> 8) as u8);
        }
        if !padded(index + 3) {
            out.push(value as u8);
        }
        index += 4;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(bytes: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut index = 0;
        while index + 3 <= bytes.len() {
            let value = (u32::from(bytes[index]) << 16)
                | (u32::from(bytes[index + 1]) << 8)
                | u32::from(bytes[index + 2]);
            for shift in [18, 12, 6, 0] {
                out.push(TABLE[((value >> shift) & 63) as usize] as char);
            }
            index += 3;
        }
        let rest = &bytes[index..];
        if rest.len() == 1 {
            let value = u32::from(rest[0]) << 16;
            out.push(TABLE[((value >> 18) & 63) as usize] as char);
            out.push(TABLE[((value >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        } else if rest.len() == 2 {
            let value = (u32::from(rest[0]) << 16) | (u32::from(rest[1]) << 8);
            out.push(TABLE[((value >> 18) & 63) as usize] as char);
            out.push(TABLE[((value >> 12) & 63) as usize] as char);
            out.push(TABLE[((value >> 6) & 63) as usize] as char);
            out.push('=');
        }
        out
    }

    fn good() -> String {
        encode(b"slack-bot-token")
    }

    fn bad() -> String {
        encode(b"reject:wrong-token")
    }

    fn leaked(value: &str) -> bool {
        value.contains("slack-bot-token")
            || value.contains("reject:wrong-token")
            || value.contains(&good())
            || value.contains(&bad())
    }

    fn params(id: &str, credential: &str) -> ChannelSettingsRequest {
        ChannelSettingsRequest::set(
            id,
            ChannelKind::Slack,
            "https://slack.example/api",
            "T0123",
            credential,
        )
    }

    #[test]
    fn set_connect_and_disconnect_keep_a_rejected_credential_from_replacing_state() {
        let origin = ChannelCatalog::default();
        let rejected = params("slack-main", &bad());
        let failure = rejected.apply(&origin).unwrap_err();
        assert_eq!(failure.reason, ChannelFailureReason::CredentialRejected);
        assert_eq!(failure.recovery, ChannelRecovery::ReplaceCredential);
        assert_eq!(failure.status, ChannelStatus::Error);
        assert_eq!(failure.message_key(), "error.channel.credential");
        assert!(!leaked(&serde_json::to_string(&failure).unwrap()));
        assert!(!leaked(&format!("{rejected:?}")));
        assert_eq!(origin, ChannelCatalog::default());

        let saved = params("slack-main", &good()).apply(&origin).unwrap();
        assert!(saved.persists());
        let catalog = saved.catalog();
        assert_eq!(
            catalog.view().channels[0].status,
            ChannelStatus::Disconnected
        );
        assert!(catalog.view().channels[0].credential_configured);
        assert!(catalog.view().channels[0].reason.is_none());
        assert_eq!(catalog.to_file_value()[0]["credential"], good());
        assert!(!leaked(&serde_json::to_string(&catalog.view()).unwrap()));
        assert!(!leaked(&format!("{catalog:?}")));

        let connected = ChannelSettingsRequest::connect("slack-main")
            .apply(catalog)
            .unwrap();
        assert_eq!(
            connected.catalog().view().channels[0].status,
            ChannelStatus::Connected
        );
        assert_eq!(connected.catalog().to_file_value()[0]["credential"], good());
        let prior = connected.catalog().to_file_value();
        let again = params("slack-main", &bad());
        let failure = again.apply(connected.catalog()).unwrap_err();
        assert_eq!(failure.recovery, ChannelRecovery::ReplaceCredential);
        assert!(!leaked(&serde_json::to_string(&failure).unwrap()));
        assert_eq!(connected.catalog().to_file_value(), prior);

        let disconnected = ChannelSettingsRequest::disconnect("slack-main")
            .apply(connected.catalog())
            .unwrap();
        assert_eq!(
            disconnected.catalog().view().channels[0].status,
            ChannelStatus::Disconnected
        );
        assert_eq!(
            disconnected.catalog().to_file_value()[0]["endpoint"],
            "https://slack.example/api"
        );
        assert!(matches!(
            ChannelSettingsRequest::disconnect("slack-main").apply(disconnected.catalog()),
            Ok(ChannelChange::Keep(_))
        ));

        let reloaded =
            ChannelCatalog::from_file_value(&disconnected.catalog().to_file_value()).unwrap();
        assert_eq!(reloaded.view(), disconnected.catalog().view());
        assert_eq!(
            ChannelSettingsRequest::get()
                .apply(&reloaded)
                .unwrap()
                .catalog()
                .view(),
            reloaded.view()
        );
    }

    #[test]
    fn connect_records_a_recoverable_error_without_replacing_parameters() {
        let credential = bad();
        let seeded = ChannelCatalog::from_file_value(&serde_json::json!([{
            "id": "slack-main",
            "kind": "slack",
            "endpoint": "https://slack.example/api",
            "account": "T0123",
            "credential": credential,
            "status": "disconnected"
        }]))
        .unwrap();
        let errored = ChannelSettingsRequest::connect("slack-main")
            .apply(&seeded)
            .unwrap();
        let channel = &errored.catalog().view().channels[0];
        assert_eq!(channel.status, ChannelStatus::Error);
        assert_eq!(channel.reason, Some(ChannelReason::CredentialRejected));
        assert_eq!(channel.recovery, Some(ChannelRecovery::ReplaceCredential));
        assert_eq!(channel.endpoint, "https://slack.example/api");
        assert_eq!(channel.account, "T0123");
        assert_eq!(
            errored.catalog().to_file_value()[0]["credential"],
            credential
        );
        assert!(!leaked(
            &serde_json::to_string(&errored.catalog().view()).unwrap()
        ));
        assert!(matches!(
            ChannelSettingsRequest::connect("slack-main").apply(errored.catalog()),
            Ok(ChannelChange::Keep(_))
        ));
        let fixed = params("slack-main", &good())
            .apply(errored.catalog())
            .unwrap();
        assert_eq!(
            fixed.catalog().view().channels[0].status,
            ChannelStatus::Disconnected
        );
        assert!(fixed.catalog().view().channels[0].reason.is_none());
        let connected = ChannelSettingsRequest::connect("slack-main")
            .apply(fixed.catalog())
            .unwrap();
        assert_eq!(
            connected.catalog().view().channels[0].status,
            ChannelStatus::Connected
        );
        let cleared = ChannelSettingsRequest::disconnect("slack-main")
            .apply(connected.catalog())
            .unwrap();
        assert_eq!(
            cleared.catalog().view().channels[0].status,
            ChannelStatus::Disconnected
        );
        assert!(cleared.catalog().view().channels[0].recovery.is_none());
    }

    #[test]
    fn rejected_commands_name_recovery_and_leave_the_catalog() {
        let saved = params("slack-main", &good())
            .apply(&ChannelCatalog::default())
            .unwrap();
        let origin = saved.catalog().clone();
        let file = origin.to_file_value();
        let cases = [
            (
                params("", &good()),
                ChannelFailureReason::InvalidParameters,
                ChannelRecovery::EditParameters,
            ),
            (
                params("slack-main", "===="),
                ChannelFailureReason::CredentialRejected,
                ChannelRecovery::ReplaceCredential,
            ),
            (
                params("slack-main", "not an envelope"),
                ChannelFailureReason::CredentialRejected,
                ChannelRecovery::ReplaceCredential,
            ),
            (
                ChannelSettingsRequest::set(
                    "slack-main",
                    ChannelKind::Slack,
                    "http://slack.example/api",
                    "T0123",
                    &good(),
                ),
                ChannelFailureReason::InvalidParameters,
                ChannelRecovery::EditParameters,
            ),
            (
                ChannelSettingsRequest::set(
                    "slack-main",
                    ChannelKind::Feishu,
                    "https://slack.example/api",
                    "  ",
                    &good(),
                ),
                ChannelFailureReason::InvalidParameters,
                ChannelRecovery::EditParameters,
            ),
            (
                ChannelSettingsRequest::connect("missing"),
                ChannelFailureReason::UnknownChannel,
                ChannelRecovery::EditParameters,
            ),
            (
                ChannelSettingsRequest::disconnect("../slack"),
                ChannelFailureReason::InvalidParameters,
                ChannelRecovery::EditParameters,
            ),
        ];
        for (request, reason, recovery) in cases {
            let failure = request.apply(&origin).unwrap_err();
            assert_eq!(failure.status, ChannelStatus::Error);
            assert_eq!(failure.reason, reason);
            assert_eq!(failure.recovery, recovery);
            assert_eq!(origin.to_file_value(), file);
            assert!(!leaked(&serde_json::to_string(&failure).unwrap()));
            assert!(!leaked(&format!("{request:?}")));
        }
        assert_eq!(
            ChannelSettingsRequest::connect("missing")
                .apply(&origin)
                .unwrap_err()
                .reason,
            ChannelFailureReason::UnknownChannel
        );
        assert_eq!(
            params("slack-main", &bad())
                .apply(&origin)
                .unwrap_err()
                .recovery,
            ChannelRecovery::ReplaceCredential
        );
        assert!(
            serde_json::from_value::<ChannelSettingsRequest>(serde_json::json!({
                "method": "settings.channel.set",
                "id": "slack-main",
                "kind": "slack",
                "endpoint": "https://slack.example/api",
                "account": "T0123",
                "credential": good(),
                "token": "secret"
            }))
            .is_err()
        );
        assert!(
            ChannelCatalog::from_file_value(&serde_json::json!({"credential": good()})).is_err()
        );
        assert_eq!(origin.to_file_value(), file);
    }
}
