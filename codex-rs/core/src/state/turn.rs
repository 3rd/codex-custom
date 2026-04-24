//! Turn-scoped state and active turn metadata scaffolding.

use codex_sandboxing::policy_transforms::merge_permission_profiles;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::approvals::InteractiveRequestId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::mcp::RequestId;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::oneshot;

use crate::session::turn_context::TurnContext;
use crate::tasks::AnySessionTask;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxEnforcement;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TokenUsage;

/// Metadata about the currently running turn.
pub(crate) struct ActiveTurn {
    pub(crate) tasks: IndexMap<String, RunningTask>,
    pub(crate) turn_state: Arc<Mutex<TurnState>>,
}

/// Whether mailbox deliveries should still be folded into the current turn.
///
/// State machine:
/// - A turn starts in `CurrentTurn`, so queued child mail can join the next
///   model request for that turn.
/// - After user-visible terminal output is recorded, we switch to `NextTurn`
///   to leave late child mail queued instead of extending an already shown
///   answer.
/// - If the same task later gets explicit same-turn work again (a steered user
///   prompt or a tool call after an untagged preamble), we reopen `CurrentTurn`
///   so that pending child mail is drained into that follow-up request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MailboxDeliveryPhase {
    /// Incoming mailbox messages can still be consumed by the current turn.
    #[default]
    CurrentTurn,
    /// The current turn already emitted visible final answer text; mailbox
    /// messages should remain queued for a later turn.
    NextTurn,
}

impl Default for ActiveTurn {
    fn default() -> Self {
        Self {
            tasks: IndexMap::new(),
            turn_state: Arc::new(Mutex::new(TurnState::default())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskKind {
    Regular,
    Review,
    Compact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeTurnPermissionsSnapshot {
    pub(crate) approval_policy: AskForApproval,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
    pub(crate) permission_profile: PermissionProfile,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub(crate) network_sandbox_policy: NetworkSandboxPolicy,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeTurnPermissionsHandle {
    snapshot: Arc<RwLock<RuntimeTurnPermissionsSnapshot>>,
}

impl RuntimeTurnPermissionsHandle {
    pub(crate) fn new(snapshot: RuntimeTurnPermissionsSnapshot) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
        }
    }

    pub(crate) fn snapshot(&self) -> RuntimeTurnPermissionsSnapshot {
        match self.snapshot.read() {
            Ok(snapshot) => snapshot.clone(),
            Err(_) => panic!("runtime turn permissions lock poisoned"),
        }
    }

    pub(crate) fn replace(&self, snapshot: RuntimeTurnPermissionsSnapshot) {
        match self.snapshot.write() {
            Ok(mut guard) => *guard = snapshot,
            Err(_) => panic!("runtime turn permissions lock poisoned"),
        }
    }
}

pub(crate) struct RunningTask {
    pub(crate) done: Arc<Notify>,
    pub(crate) kind: TaskKind,
    pub(crate) task: Arc<dyn AnySessionTask>,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) handle: AbortOnDropHandle<()>,
    pub(crate) turn_context: Arc<TurnContext>,
    // Timer recorded when the task drops to capture the full turn duration.
    pub(crate) _timer: Option<codex_otel::Timer>,
}

pub(crate) struct RemovedTask {
    pub(crate) records_turn_token_usage_on_span: bool,
    pub(crate) active_turn_is_empty: bool,
}

pub(crate) struct PendingApprovalRequest {
    pub(crate) request: InteractiveRequestId,
    pub(crate) tx: oneshot::Sender<ReviewDecision>,
}

impl PendingApprovalRequest {
    fn interactive_request_id(&self) -> InteractiveRequestId {
        self.request.clone()
    }
}

pub(crate) struct PendingElicitationRequest {
    pub(crate) event: ElicitationRequestEvent,
    pub(crate) tx: oneshot::Sender<ElicitationResponse>,
}

pub(crate) enum PendingInteractiveResolution {
    Approval {
        request: InteractiveRequestId,
        decision: ReviewDecision,
        tx: oneshot::Sender<ReviewDecision>,
    },
    RequestPermissions {
        request: InteractiveRequestId,
        response: RequestPermissionsResponse,
        tx: oneshot::Sender<RequestPermissionsResponse>,
    },
    Elicitation {
        request: InteractiveRequestId,
        response: ElicitationResponse,
        tx: oneshot::Sender<ElicitationResponse>,
    },
}

impl ActiveTurn {
    pub(crate) fn add_task(&mut self, task: RunningTask) {
        let sub_id = task.turn_context.sub_id.clone();
        self.tasks.insert(sub_id, task);
    }

    pub(crate) fn remove_task(&mut self, sub_id: &str) -> Option<RemovedTask> {
        let task = self.tasks.swap_remove(sub_id)?;
        let records_turn_token_usage_on_span = task.task.records_turn_token_usage_on_span();
        task.handle.detach();
        Some(RemovedTask {
            records_turn_token_usage_on_span,
            active_turn_is_empty: self.tasks.is_empty(),
        })
    }

    pub(crate) fn drain_tasks(&mut self) -> Vec<RunningTask> {
        self.tasks.drain(..).map(|(_, task)| task).collect()
    }
}

/// Mutable state for a single turn.
#[derive(Default)]
pub(crate) struct TurnState {
    pending_approvals: HashMap<String, PendingApprovalRequest>,
    pending_request_permissions: HashMap<String, PendingRequestPermissions>,
    pending_user_input: HashMap<String, oneshot::Sender<RequestUserInputResponse>>,
    pending_elicitations: HashMap<(String, RequestId), PendingElicitationRequest>,
    pending_dynamic_tools: HashMap<String, oneshot::Sender<DynamicToolResponse>>,
    pending_input: Vec<ResponseInputItem>,
    mailbox_delivery_phase: MailboxDeliveryPhase,
    granted_permissions: Option<AdditionalPermissionProfile>,
    strict_auto_review_enabled: bool,
    pub(crate) tool_calls: u64,
    pub(crate) has_memory_citation: bool,
    pub(crate) token_usage_at_turn_start: TokenUsage,
}

pub(crate) struct PendingRequestPermissions {
    pub(crate) tx_response: oneshot::Sender<RequestPermissionsResponse>,
    pub(crate) requested_permissions: RequestPermissionProfile,
    pub(crate) cwd: AbsolutePathBuf,
}

impl TurnState {
    pub(crate) fn insert_pending_approval(
        &mut self,
        key: String,
        request: PendingApprovalRequest,
    ) -> Option<PendingApprovalRequest> {
        self.pending_approvals.insert(key, request)
    }

    pub(crate) fn remove_pending_approval(&mut self, key: &str) -> Option<PendingApprovalRequest> {
        self.pending_approvals.remove(key)
    }

    pub(crate) fn clear_pending_interactive_requests(&mut self) {
        self.pending_approvals.clear();
        self.pending_request_permissions.clear();
        self.pending_user_input.clear();
        self.pending_elicitations.clear();
        self.pending_dynamic_tools.clear();
    }

    pub(crate) fn clear_pending(&mut self) {
        self.clear_pending_interactive_requests();
    }

    pub(crate) fn insert_pending_request_permissions(
        &mut self,
        key: String,
        pending_request_permissions: PendingRequestPermissions,
    ) -> Option<PendingRequestPermissions> {
        self.pending_request_permissions
            .insert(key, pending_request_permissions)
    }

    pub(crate) fn remove_pending_request_permissions(
        &mut self,
        key: &str,
    ) -> Option<PendingRequestPermissions> {
        self.pending_request_permissions.remove(key)
    }

    pub(crate) fn insert_pending_user_input(
        &mut self,
        key: String,
        tx: oneshot::Sender<RequestUserInputResponse>,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.insert(key, tx)
    }

    pub(crate) fn remove_pending_user_input(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.remove(key)
    }

    pub(crate) fn insert_pending_elicitation(
        &mut self,
        server_name: String,
        request_id: RequestId,
        request: PendingElicitationRequest,
    ) -> Option<PendingElicitationRequest> {
        self.pending_elicitations
            .insert((server_name, request_id), request)
    }

    pub(crate) fn remove_pending_elicitation(
        &mut self,
        server_name: &str,
        request_id: &RequestId,
    ) -> Option<PendingElicitationRequest> {
        self.pending_elicitations
            .remove(&(server_name.to_string(), request_id.clone()))
    }

    pub(crate) fn insert_pending_dynamic_tool(
        &mut self,
        key: String,
        tx: oneshot::Sender<DynamicToolResponse>,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.insert(key, tx)
    }

    pub(crate) fn remove_pending_dynamic_tool(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.remove(key)
    }

    pub(crate) fn push_pending_input(&mut self, input: ResponseInputItem) {
        self.pending_input.push(input);
    }

    pub(crate) fn prepend_pending_input(&mut self, mut input: Vec<ResponseInputItem>) {
        if input.is_empty() {
            return;
        }

        input.append(&mut self.pending_input);
        self.pending_input = input;
    }

    pub(crate) fn take_pending_input(&mut self) -> Vec<ResponseInputItem> {
        if self.pending_input.is_empty() {
            Vec::with_capacity(0)
        } else {
            let mut ret = Vec::new();
            std::mem::swap(&mut ret, &mut self.pending_input);
            ret
        }
    }

    pub(crate) fn has_pending_input(&self) -> bool {
        !self.pending_input.is_empty()
    }

    pub(crate) fn accept_mailbox_delivery_for_current_turn(&mut self) {
        self.set_mailbox_delivery_phase(MailboxDeliveryPhase::CurrentTurn);
    }

    pub(crate) fn accepts_mailbox_delivery_for_current_turn(&self) -> bool {
        self.mailbox_delivery_phase == MailboxDeliveryPhase::CurrentTurn
    }

    pub(crate) fn set_mailbox_delivery_phase(&mut self, phase: MailboxDeliveryPhase) {
        self.mailbox_delivery_phase = phase;
    }

    pub(crate) fn record_granted_permissions(&mut self, permissions: AdditionalPermissionProfile) {
        self.granted_permissions =
            merge_permission_profiles(self.granted_permissions.as_ref(), Some(&permissions));
    }

    pub(crate) fn granted_permissions(&self) -> Option<AdditionalPermissionProfile> {
        self.granted_permissions.clone()
    }

    pub(crate) fn enable_strict_auto_review(&mut self) {
        self.strict_auto_review_enabled = true;
    }

    pub(crate) fn strict_auto_review_enabled(&self) -> bool {
        self.strict_auto_review_enabled
    }

    pub(crate) fn reevaluate_runtime_permissions(
        &mut self,
        runtime_permissions: &RuntimeTurnPermissionsSnapshot,
    ) -> Vec<PendingInteractiveResolution> {
        let mut resolved = Vec::new();

        let pending_approvals = std::mem::take(&mut self.pending_approvals);
        for (key, request) in pending_approvals {
            let decision = if matches!(
                runtime_permissions.sandbox_policy,
                SandboxPolicy::DangerFullAccess
            ) {
                Some(ReviewDecision::Approved)
            } else {
                match runtime_permissions.approval_policy {
                    AskForApproval::Never => Some(ReviewDecision::Abort),
                    AskForApproval::Granular(granular) if !granular.allows_sandbox_approval() => {
                        Some(ReviewDecision::Abort)
                    }
                    AskForApproval::UnlessTrusted
                    | AskForApproval::OnFailure
                    | AskForApproval::OnRequest
                    | AskForApproval::Granular(_) => None,
                }
            };

            match (request, decision) {
                (request, Some(decision)) => {
                    let request_id = request.interactive_request_id();
                    resolved.push(PendingInteractiveResolution::Approval {
                        request: request_id,
                        decision,
                        tx: request.tx,
                    });
                }
                (request, None) => {
                    self.pending_approvals.insert(key, request);
                }
            }
        }

        let pending_request_permissions = std::mem::take(&mut self.pending_request_permissions);
        for (key, request) in pending_request_permissions {
            let response = match runtime_permissions.approval_policy {
                AskForApproval::Never => Some(RequestPermissionsResponse {
                    permissions: RequestPermissionProfile::default(),
                    scope: PermissionGrantScope::Turn,
                    strict_auto_review: false,
                }),
                AskForApproval::Granular(granular) if !granular.allows_request_permissions() => {
                    Some(RequestPermissionsResponse {
                        permissions: RequestPermissionProfile::default(),
                        scope: PermissionGrantScope::Turn,
                        strict_auto_review: false,
                    })
                }
                AskForApproval::UnlessTrusted
                | AskForApproval::OnFailure
                | AskForApproval::OnRequest
                | AskForApproval::Granular(_) => None,
            };

            match response {
                Some(response) => {
                    resolved.push(PendingInteractiveResolution::RequestPermissions {
                        request: InteractiveRequestId::RequestPermissions { call_id: key },
                        response,
                        tx: request.tx_response,
                    });
                }
                None => {
                    self.pending_request_permissions.insert(key, request);
                }
            }
        }

        let pending_elicitations = std::mem::take(&mut self.pending_elicitations);
        for (key, request) in pending_elicitations {
            let response = match runtime_permissions.approval_policy {
                AskForApproval::Never => Some(ElicitationResponse {
                    action: ElicitationAction::Cancel,
                    content: None,
                    meta: None,
                }),
                AskForApproval::Granular(granular) if !granular.allows_mcp_elicitations() => {
                    Some(ElicitationResponse {
                        action: ElicitationAction::Cancel,
                        content: None,
                        meta: None,
                    })
                }
                AskForApproval::UnlessTrusted
                | AskForApproval::OnFailure
                | AskForApproval::OnRequest
                | AskForApproval::Granular(_) => None,
            };

            match response {
                Some(response) => {
                    resolved.push(PendingInteractiveResolution::Elicitation {
                        request: InteractiveRequestId::McpElicitation {
                            server_name: request.event.server_name.clone(),
                            request_id: request.event.id.clone(),
                        },
                        response,
                        tx: request.tx,
                    });
                }
                None => {
                    self.pending_elicitations.insert(key, request);
                }
            }
        }

        resolved
    }
}

impl ActiveTurn {
    /// Clear any pending interactive requests buffered for the current turn.
    pub(crate) async fn clear_pending_interactive_requests(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending_interactive_requests();
    }

    pub(crate) async fn clear_pending(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::approvals::ElicitationRequest;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn runtime_permissions(
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) -> RuntimeTurnPermissionsSnapshot {
        let file_system_sandbox_policy = FileSystemSandboxPolicy::unrestricted();
        let network_sandbox_policy = NetworkSandboxPolicy::Enabled;
        let permission_profile = PermissionProfile::from_runtime_permissions_with_enforcement(
            SandboxEnforcement::from_legacy_sandbox_policy(&sandbox_policy),
            &file_system_sandbox_policy,
            network_sandbox_policy,
        );
        RuntimeTurnPermissionsSnapshot {
            approval_policy,
            approvals_reviewer: ApprovalsReviewer::default(),
            permission_profile,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            windows_sandbox_level: WindowsSandboxLevel::default(),
        }
    }

    #[test]
    fn reevaluate_runtime_permissions_cancels_pending_elicitation_when_policy_disallows_it() {
        let mut turn_state = TurnState::default();
        let (tx, _rx) = oneshot::channel();
        let request_id = RequestId::String("req-1".to_string());
        turn_state.insert_pending_elicitation(
            "server".to_string(),
            request_id.clone(),
            PendingElicitationRequest {
                event: ElicitationRequestEvent {
                    turn_id: Some("turn-1".to_string()),
                    server_name: "server".to_string(),
                    id: request_id.clone(),
                    request: ElicitationRequest::Form {
                        meta: None,
                        message: "need approval".to_string(),
                        requested_schema: json!({"type": "object"}),
                    },
                },
                tx,
            },
        );

        let resolved = turn_state.reevaluate_runtime_permissions(&runtime_permissions(
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        ));

        assert_eq!(resolved.len(), 1);
        match &resolved[0] {
            PendingInteractiveResolution::Elicitation {
                request, response, ..
            } => {
                assert_eq!(
                    request,
                    &InteractiveRequestId::McpElicitation {
                        server_name: "server".to_string(),
                        request_id,
                    }
                );
                assert_eq!(response.action, ElicitationAction::Cancel);
                assert_eq!(response.content, None);
            }
            _ => panic!("expected elicitation resolution"),
        }
    }
}
