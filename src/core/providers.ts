/** Provider connection contract v1. Secrets and service wire formats never cross this boundary. */
export interface Provider {
 id:string;name:string;apiStyle?:string;adapterVersion?:string;availability:'IMPLEMENTED'|'MOCK_ONLY'|'ADAPTER_REQUIRED';
 authMethods:{id:string;label:string;implemented:boolean;fields:{id:string;label:string;options:string[]}[]}[];
 credentialModes:('existingEnv'|'nativeVault'|'mock')[];credentialEnvField?:string;
 networks?:string;planLimitations:string;oauth?:string;capabilities:Record<string,boolean>;scope?:string;
}
export interface Workspace {id:string;name:string}
export interface Channel {id:string;workspaceId:string;name:string;platform:string;queuePaused:boolean}
export interface ConnectionCheck {id:string;startedAt:string;endedAt:string|null;scope:string;status:string;authenticated:boolean;targetsVerified:boolean;error:{code:string;message:string}|null;evidence:{operation:string;httpStatus:number|null;requestSha256:string;responseSha256:string|null;result:string}[];externalWrites:0}
export interface ProviderConnection {
 id:string;providerId:string;revision:number;credentialRef:string;authMethod:string;fields:Record<string,string>;state:string;
 accountId:string|null;workspaces:Workspace[];channels:Channel[];selectedWorkspaceId:string|null;selectedChannelIds:string[];checks:ConnectionCheck[];configuredAt:string;
}
export interface ProviderState {version:1;providers:Provider[];connections:ProviderConnection[];externalWrites:0;publication:'NOT_PERFORMED'}
export type CredentialSource={kind:'existingEnv';path:string;field:string}|{kind:'nativeVault'}|{kind:'mock'};
/** Reference-only common authentication binding. OAuth is a future contract, never an enabled connection in v1. */
export type ProviderAuthentication={kind:'apiKey';credentialRef:string}|{kind:'oauth2';clientId:string;accessTokenRef:string;refreshTokenRef:string|null;expiresAt:string|null;grantedScopes:string[]}|{kind:'providerDefined';methodId:string;credentialRef:string};
export interface FutureOAuthAuthorization {providerId:string;clientId:string;registeredRedirectUri:string;requestedScopes:string[];pkceMethod:'S256';stateRef:string;verifierRef:string}
export type ProviderAction={operation:'get'|'listProviders'}|{operation:'getProviderCapabilities';providerId:string}|{operation:'connect';providerId:string;authMethod:string;source:CredentialSource;fields:Record<string,string>}|{operation:'testConnection'|'disconnect';connectionId:string;revision:number}|{operation:'saveTargets';connectionId:string;revision:number;workspaceId:string;channelIds:string[]}|{operation:'getConnectionStatus'|'listWorkspaces';connectionId:string}|{operation:'listChannels';connectionId:string;workspaceId:string};
export const providerStateLabels:Record<string,string>={CONFIGURED:'設定済み・API未確認',CHECKING_UNKNOWN:'確認開始済み・結果未確定',AUTHENTICATED:'認証確認済み・投稿先なし',TARGETS_VERIFIED:'API認証・投稿先取得を確認済み',MOCK_VERIFIED:'模擬確認済み（実接続ではありません）',DISCONNECTED:'切断済み',AUTHENTICATION_FAILED:'認証失敗・再認証が必要',CREDENTIAL_UNAVAILABLE:'資格情報を利用できません',FORBIDDEN:'権限不足',RATE_LIMITED:'API利用制限',UNKNOWN:'結果不明・停止中',GRAPHQL_ERROR:'API応答エラー'};
export const canSaveTargets=(c:ProviderConnection)=>['TARGETS_VERIFIED','MOCK_VERIFIED'].includes(c.state);
export const canTestConnection=(c:ProviderConnection)=>!['DISCONNECTED','CHECKING_UNKNOWN','UNKNOWN'].includes(c.state);
export type PostState='PREPARED'|'APPROVAL_REQUIRED'|'APPROVED'|'SUBMISSION_STARTED'|'PROVIDER_ACCEPTED'|'SCHEDULED'|'PUBLISHED_VERIFIED'|'FAILED'|'UNKNOWN';
export interface PostBinding {jobId:string;providerId:string;connectionId:string;workspaceId:string;channelId:string;platform:string;text:string;textSha256:string;method:'draft'|'schedule'|'publish';scheduledAt:string|null}
export interface FuturePost {binding:PostBinding;publicationApproval:{approvalId:string;binding:PostBinding}|null;state:PostState;externalReceiptId:string|null;providerStatus:string|null;publicationCheck:string;error:{code:string;message:string}|null;submissionRecordId:string|null;providerIdempotencyKey:string|null}
// Type-level design only. Core exposes no implementation or Action for these methods.
export interface FuturePostingAdapter {createDraft(p:FuturePost):Promise<FuturePost>;schedulePost(p:FuturePost):Promise<FuturePost>;publishPost(p:FuturePost):Promise<FuturePost>;getPostStatus(p:FuturePost):Promise<FuturePost>}
