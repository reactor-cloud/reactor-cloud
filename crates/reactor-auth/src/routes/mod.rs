//! HTTP route handlers for the auth service.

pub mod api_keys;
pub mod authorize;
pub mod health;
pub mod invitations;
pub mod keys;
pub mod login;
pub mod logout;
pub mod members;
pub mod operators;
pub mod orgs;
pub mod password_reset;
pub mod permissions;
pub mod signup;
pub mod token;
pub mod user;
pub mod verify;

pub use api_keys::{
    create_api_key, list_api_keys, revoke_api_key, ApiKeyResponse, CreateApiKeyRequest,
    CreateApiKeyResponse, ListApiKeysResponse, RevokeApiKeyResponse,
};
pub use authorize::{authorize, authorize_submit, AuthorizeQuery, AuthorizeFormRequest};
pub use health::health;
pub use invitations::{
    accept_invitation, create_invitation, delete_invitation, list_invitations,
    CreateInvitationRequest, InvitationResponse,
};
pub use keys::{jwks, openid_configuration, KeysState};
pub use login::{login, LoginRequest, LoginResponse};
pub use logout::logout;
pub use members::{
    delete_member, get_member, list_members, update_member, MemberResponse, UpdateMemberRequest,
};
pub use operators::{
    bootstrap_operator, operators_status, promote_operator,
    BootstrapOperatorRequest, BootstrapOperatorResponse,
    OperatorsStatusResponse, PromoteOperatorRequest, PromoteOperatorResponse,
};
pub use orgs::{
    create_org, delete_org, get_org, list_orgs, list_roles, update_org, CreateOrgRequest,
    OrgResponse, UpdateOrgRequest,
};
pub use permissions::{
    check_permissions, get_permissions, resolve_ctx, CheckPermissionsRequest, PermissionsResponse,
    ResolveCtxResponse,
};
pub use signup::{signup, SessionResponse, SignupRequest, SignupResponse, UserResponse};
pub use token::{token, TokenQuery, TokenResponse};
pub use user::{delete_user, get_me, get_user, update_user, CurrentUserResponse, UpdateUserRequest};
pub use password_reset::{
    confirm_password_reset, request_password_reset, PasswordResetConfirmBody,
    PasswordResetConfirmResponse, PasswordResetRequestBody, PasswordResetRequestResponse,
};
pub use verify::{resend_verification, verify_email, ResendRequest, ResendResponse, VerifyQuery, VerifyResponse};
