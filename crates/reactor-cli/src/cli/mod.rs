//! CLI command definitions using clap derive.

use crate::output::OutputFormat;
use clap::{Parser, Subcommand};

/// Reactor CLI - manage Reactor servers and projects.
#[derive(Debug, Parser)]
#[command(name = "reactor")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Context to use (overrides REACTOR_CONTEXT and manifest default).
    #[arg(long, short = 'c', global = true, env = "REACTOR_CONTEXT")]
    pub context: Option<String>,

    /// Path to project manifest (default: find reactor.toml in cwd or parents).
    #[arg(long, short = 'm', global = true)]
    pub manifest: Option<std::path::PathBuf>,

    /// Output format (human, json). Auto-detects: human for TTY, json otherwise.
    #[arg(long, short = 'o', global = true, env = "REACTOR_OUTPUT")]
    pub output: Option<OutputFormat>,

    /// Skip confirmation prompts.
    #[arg(long, global = true, env = "REACTOR_ASSUME_YES")]
    pub yes: bool,

    /// Enable verbose output.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Authentication token (overrides context token).
    #[arg(long, global = true, env = "REACTOR_TOKEN")]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize a new Reactor project.
    Init(InitArgs),

    /// Show project information.
    Project(ProjectArgs),

    /// Manage contexts (server connections).
    Context(ContextArgs),

    /// Authenticate with a Reactor server.
    Login(LoginArgs),

    /// Remove stored authentication.
    Logout(LogoutArgs),

    /// Show current user information.
    Whoami(WhoamiArgs),

    /// Run diagnostics on the project and server.
    Doctor(DoctorArgs),

    /// Show version information.
    Version(VersionArgs),

    /// Run database migrations.
    Migrate(MigrateArgs),

    /// Build a deployment bundle.
    Build(BuildArgs),

    /// Deploy a project to a Reactor server.
    Deploy(DeployArgs),

    /// Manage functions.
    Functions(FunctionsArgs),

    /// Manage sites.
    Sites(SitesArgs),

    /// Manage background jobs.
    Jobs(JobsArgs),

    /// Data operations.
    Data(DataArgs),

    /// AI inference operations.
    Ai(AiArgs),

    /// Authentication and organization management.
    Auth(AuthArgs),

    /// Manage vault secrets.
    Vault(VaultArgs),

    /// Cloud control plane management.
    Cloud(CloudArgs),

    /// Manage connectors and integrations.
    Connect(ConnectArgs),

    /// Inspect a resource.
    Inspect(InspectArgs),

    /// View logs.
    Logs(LogsArgs),

    /// Generate TypeScript types from database schema.
    Types(TypesArgs),

    /// Start a local development server.
    #[cfg(feature = "dev")]
    Dev(DevArgs),

    /// Start a detached local server.
    #[cfg(feature = "dev")]
    Up(UpArgs),

    /// Stop a detached local server.
    #[cfg(feature = "dev")]
    Down(DownArgs),

    /// Show status of a local server.
    #[cfg(feature = "dev")]
    Status(StatusArgs),
}

// ============================================================================
// Argument structs for each command
// ============================================================================

#[derive(Debug, Parser)]
pub struct InitArgs {
    /// Project name.
    pub name: String,

    /// Overwrite existing files.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommands,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommands {
    /// Show project manifest and paths.
    Show,
}

#[derive(Debug, Parser)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextCommands,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommands {
    /// List available contexts.
    List,

    /// Add a new context.
    Add {
        /// Context name.
        name: String,

        /// Server endpoint URL.
        #[arg(long)]
        endpoint: String,

        /// Organization slug.
        #[arg(long)]
        org: Option<String>,

        /// Environment variable containing the token.
        #[arg(long, conflicts_with = "token")]
        token_env: Option<String>,

        /// Authentication token.
        #[arg(long, conflicts_with = "token_env")]
        token: Option<String>,
    },

    /// Set the default context.
    Use {
        /// Context name.
        name: String,
    },

    /// Show context details.
    Show {
        /// Context name (default: current context).
        name: Option<String>,
    },

    /// Remove a context.
    Remove {
        /// Context name.
        name: String,
    },
}

#[derive(Debug, Parser)]
pub struct LoginArgs {
    /// Context to authenticate.
    #[arg(long)]
    pub context: Option<String>,

    /// Token to store (legacy mode).
    #[arg(long)]
    pub token: Option<String>,

    /// Use browser-based PKCE OAuth flow (recommended for operators).
    #[arg(long)]
    pub browser: bool,

    /// Skip browser and use device code flow (for headless environments).
    #[arg(long, requires = "browser")]
    pub no_browser: bool,

    /// Bootstrap as the first platform operator (requires loopback + admin token).
    #[arg(long)]
    pub bootstrap: bool,

    /// Store tokens in ~/.reactor/tokens.toml instead of OS keychain.
    /// Avoids macOS keychain password prompts with unsigned binaries.
    #[arg(long)]
    pub file_storage: bool,
}

#[derive(Debug, Parser)]
pub struct LogoutArgs {
    /// Context to log out from.
    #[arg(long)]
    pub context: Option<String>,
}

#[derive(Debug, Parser)]
pub struct WhoamiArgs {}

#[derive(Debug, Parser)]
pub struct DoctorArgs {}

#[derive(Debug, Parser)]
pub struct VersionArgs {}

#[derive(Debug, Parser)]
pub struct MigrateArgs {
    /// Show migration plan without applying.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
pub struct BuildArgs {
    /// Output path for the bundle.
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
}

#[derive(Debug, Parser)]
pub struct DeployArgs {
    /// Path to a pre-built bundle.
    #[arg(long)]
    pub bundle: Option<std::path::PathBuf>,

    /// Skip building, use existing bundle.
    #[arg(long)]
    pub prebuilt: bool,
}

#[derive(Debug, Parser)]
pub struct FunctionsArgs {
    #[command(subcommand)]
    pub command: FunctionsCommands,
}

#[derive(Debug, Subcommand)]
pub enum FunctionsCommands {
    /// List functions.
    List,

    /// Show function details.
    Show {
        /// Function name.
        name: String,
    },

    /// Deploy a function.
    Deploy {
        /// Function name.
        name: String,

        /// Source directory.
        #[arg(long)]
        source: Option<std::path::PathBuf>,
    },

    /// Rollback to a previous deployment.
    Rollback {
        /// Function name.
        name: String,

        /// Deployment ID to rollback to.
        #[arg(long)]
        to: String,
    },

    /// Invoke a function.
    Invoke {
        /// Function name.
        name: String,

        /// Request data (JSON or @file.json).
        #[arg(long)]
        data: Option<String>,
    },

    /// Manage environment variables.
    Env(FunctionsEnvArgs),

    /// View function logs.
    Logs {
        /// Function name.
        name: String,

        /// Show logs since duration (e.g., 1h, 30m).
        #[arg(long)]
        since: Option<String>,

        /// Follow logs (polling).
        #[arg(long, short = 'f')]
        follow: bool,
    },
}

#[derive(Debug, Parser)]
pub struct FunctionsEnvArgs {
    #[command(subcommand)]
    pub command: FunctionsEnvCommands,
}

#[derive(Debug, Subcommand)]
pub enum FunctionsEnvCommands {
    /// List environment variables.
    List {
        /// Function name.
        name: String,
    },

    /// Get an environment variable.
    Get {
        /// Function name.
        name: String,

        /// Variable key.
        key: String,
    },

    /// Set an environment variable.
    Set {
        /// Function name.
        name: String,

        /// Variable key.
        key: String,

        /// Variable value.
        value: String,
    },

    /// Unset an environment variable.
    Unset {
        /// Function name.
        name: String,

        /// Variable key.
        key: String,
    },
}

#[derive(Debug, Parser)]
pub struct SitesArgs {
    #[command(subcommand)]
    pub command: SitesCommands,
}

#[derive(Debug, Subcommand)]
pub enum SitesCommands {
    /// List sites.
    List,

    /// Show site details.
    Show {
        /// Site name.
        name: String,
    },

    /// Deploy a site.
    Deploy {
        /// Site name.
        name: String,

        /// Source directory.
        #[arg(long)]
        source: Option<std::path::PathBuf>,
    },

    /// Promote a deployment.
    Promote {
        /// Site name.
        name: String,

        /// Deployment ID.
        #[arg(long)]
        deployment: String,
    },

    /// Rollback to the previous deployment.
    Rollback {
        /// Site name.
        name: String,
    },

    /// Manage custom domains.
    Domains(SitesDomainsArgs),

    /// Revalidate ISR cache.
    Revalidate {
        /// Site name.
        name: String,

        /// Path to revalidate.
        #[arg(long)]
        path: String,
    },

    /// View site logs.
    Logs {
        /// Site name.
        name: String,

        /// Show logs since duration.
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub struct SitesDomainsArgs {
    #[command(subcommand)]
    pub command: SitesDomainsCommands,
}

#[derive(Debug, Subcommand)]
pub enum SitesDomainsCommands {
    /// List domains.
    List {
        /// Site name.
        name: String,
    },

    /// Add a domain.
    Add {
        /// Site name.
        name: String,

        /// Domain to add.
        domain: String,
    },

    /// Remove a domain.
    Remove {
        /// Site name.
        name: String,

        /// Domain ID.
        domain_id: String,
    },

    /// Verify a domain.
    Verify {
        /// Site name.
        name: String,

        /// Domain ID.
        domain_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct JobsArgs {
    #[command(subcommand)]
    pub command: JobsCommands,
}

#[derive(Debug, Subcommand)]
pub enum JobsCommands {
    /// List jobs.
    List,

    /// Show job details.
    Show {
        /// Job name.
        name: String,
    },

    /// Trigger a job manually.
    Trigger {
        /// Job name.
        name: String,

        /// Job data (JSON or @file.json).
        #[arg(long)]
        data: Option<String>,
    },

    /// List job runs.
    Runs {
        /// Job name.
        name: String,

        /// Maximum number of runs to show.
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Show run details.
    Run {
        /// Run ID.
        run_id: String,
    },

    /// Manage dead letter queue.
    Dlq(JobsDlqArgs),

    /// View job logs.
    Logs {
        /// Job name.
        name: String,

        /// Show logs since duration.
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub struct JobsDlqArgs {
    #[command(subcommand)]
    pub command: JobsDlqCommands,
}

#[derive(Debug, Subcommand)]
pub enum JobsDlqCommands {
    /// List DLQ entries.
    List {
        /// Filter by job name.
        #[arg(long)]
        job: Option<String>,
    },

    /// Replay a DLQ entry.
    Replay {
        /// DLQ entry ID.
        id: String,
    },

    /// Purge DLQ entries.
    Purge {
        /// Filter by job name.
        #[arg(long)]
        job: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataCommands,
}

#[derive(Debug, Subcommand)]
pub enum DataCommands {
    /// Run data migrations.
    Migrate {
        /// Show migration plan without applying.
        #[arg(long)]
        dry_run: bool,
    },

    /// Inspect a table.
    Inspect {
        /// Table name.
        table: String,
    },

    /// Execute a SQL query.
    Query {
        /// SQL query.
        #[arg(long)]
        sql: String,

        /// Query parameters (JSON or @file.json).
        #[arg(long)]
        params: Option<String>,

        /// Allow write operations (INSERT, UPDATE, DELETE).
        #[arg(long)]
        write: bool,
    },
}

#[derive(Debug, Parser)]
pub struct AiArgs {
    #[command(subcommand)]
    pub command: AiCommands,
}

#[derive(Debug, Subcommand)]
pub enum AiCommands {
    /// List available models.
    Models(AiModelsArgs),

    /// List aliases.
    Aliases(AiAliasesArgs),

    /// Test an AI model with a prompt.
    Test(AiTestArgs),
}

#[derive(Debug, Parser)]
pub struct AiModelsArgs {
    /// Filter by capability (e.g., chat, reasoning, vision, embeddings).
    #[arg(long, short)]
    pub capability: Option<String>,
}

#[derive(Debug, Parser)]
pub struct AiAliasesArgs {}

#[derive(Debug, Parser)]
pub struct AiTestArgs {
    /// Model or alias to use.
    pub model: String,

    /// Prompt to send.
    #[arg(long, short)]
    pub prompt: Option<String>,

    /// System message.
    #[arg(long, short)]
    pub system: Option<String>,

    /// Enable streaming output.
    #[arg(long)]
    pub stream: bool,

    /// Maximum tokens to generate.
    #[arg(long)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Parser)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommands,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// Manage organizations.
    Orgs(AuthOrgsArgs),

    /// Manage organization members.
    Members(AuthMembersArgs),

    /// Manage API keys.
    Keys(AuthKeysArgs),

    /// Manage invitations.
    Invitations(AuthInvitationsArgs),
}

#[derive(Debug, Parser)]
pub struct AuthOrgsArgs {
    #[command(subcommand)]
    pub command: AuthOrgsCommands,
}

#[derive(Debug, Subcommand)]
pub enum AuthOrgsCommands {
    /// List organizations.
    List,

    /// Create an organization.
    Create {
        /// Organization name.
        #[arg(long)]
        name: String,

        /// Organization slug.
        slug: String,
    },

    /// Show organization details.
    Show {
        /// Organization ID.
        org_id: String,
    },

    /// Update an organization.
    Update {
        /// Organization ID.
        org_id: String,

        /// New name.
        #[arg(long)]
        name: Option<String>,

        /// New slug.
        #[arg(long)]
        slug: Option<String>,
    },

    /// Delete an organization.
    Delete {
        /// Organization ID.
        org_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct AuthMembersArgs {
    #[command(subcommand)]
    pub command: AuthMembersCommands,
}

#[derive(Debug, Subcommand)]
pub enum AuthMembersCommands {
    /// List members.
    List {
        /// Organization ID.
        org_id: String,
    },

    /// Add a member.
    Add {
        /// Organization ID.
        org_id: String,

        /// User ID.
        user_id: String,

        /// Role.
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Update member role.
    Update {
        /// Organization ID.
        org_id: String,

        /// User ID.
        user_id: String,

        /// New role.
        role: String,
    },

    /// Remove a member.
    Remove {
        /// Organization ID.
        org_id: String,

        /// User ID.
        user_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct AuthKeysArgs {
    #[command(subcommand)]
    pub command: AuthKeysCommands,
}

#[derive(Debug, Subcommand)]
pub enum AuthKeysCommands {
    /// List API keys.
    List {
        /// Organization ID.
        org_id: String,
    },

    /// Create an API key.
    Create {
        /// Organization ID.
        org_id: String,

        /// Key name.
        name: String,
    },

    /// Revoke an API key.
    Revoke {
        /// Key ID.
        key_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct AuthInvitationsArgs {
    #[command(subcommand)]
    pub command: AuthInvitationsCommands,
}

#[derive(Debug, Subcommand)]
pub enum AuthInvitationsCommands {
    /// List invitations.
    List {
        /// Organization ID.
        org_id: String,
    },

    /// Create an invitation.
    Create {
        /// Organization ID.
        org_id: String,

        /// Email address.
        email: String,

        /// Role.
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Revoke an invitation.
    Revoke {
        /// Invitation ID.
        invitation_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct TypesArgs {
    #[command(subcommand)]
    pub command: TypesCommands,
}

#[derive(Debug, Subcommand)]
pub enum TypesCommands {
    /// Generate TypeScript types from database schema.
    Generate {
        /// Output file path (default: ./database.types.ts).
        #[arg(long, short = 'o', default_value = "./database.types.ts")]
        output: std::path::PathBuf,
    },
}

#[derive(Debug, Parser)]
pub struct InspectArgs {
    /// Resource kind (function, site, job, org, member, key, bucket, domain).
    pub kind: String,

    /// Resource name or ID.
    pub name: String,
}

#[derive(Debug, Parser)]
pub struct LogsArgs {
    /// Capability (functions, sites, jobs).
    pub capability: String,

    /// Resource name (optional).
    pub name: Option<String>,

    /// Show logs since duration.
    #[arg(long)]
    pub since: Option<String>,

    /// Follow logs (polling).
    #[arg(long, short = 'f')]
    pub follow: bool,
}

#[cfg(feature = "dev")]
#[derive(Debug, Parser)]
pub struct DevArgs {
    /// Host to bind to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Database URL. If not provided, uses --ephemeral.
    #[arg(long, conflicts_with = "ephemeral")]
    pub db: Option<String>,

    /// Use ephemeral (docker-managed) Postgres.
    #[arg(long)]
    pub ephemeral: bool,

    /// Admin token for the server.
    #[arg(long)]
    pub admin_token: Option<String>,

    /// Context name for the local server.
    #[arg(long)]
    pub context: Option<String>,
}

#[cfg(feature = "dev")]
#[derive(Debug, Parser)]
pub struct UpArgs {
    /// Host to bind to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Database URL. If not provided, uses --ephemeral.
    #[arg(long, conflicts_with = "ephemeral")]
    pub db: Option<String>,

    /// Use ephemeral (docker-managed) Postgres.
    #[arg(long)]
    pub ephemeral: bool,

    /// Admin token for the server.
    #[arg(long)]
    pub admin_token: Option<String>,

    /// Context name for the local server.
    #[arg(long)]
    pub context: Option<String>,

    /// Force restart if already running.
    #[arg(long)]
    pub force: bool,
}

#[cfg(feature = "dev")]
#[derive(Debug, Parser)]
pub struct DownArgs {
    /// Context for the detached server.
    #[arg(long)]
    pub context: Option<String>,
}

#[cfg(feature = "dev")]
#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Context for the detached server.
    #[arg(long)]
    pub context: Option<String>,

    /// Show status of all servers.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Parser)]
pub struct VaultArgs {
    #[command(subcommand)]
    pub command: VaultCommands,
}

#[derive(Debug, Subcommand)]
pub enum VaultCommands {
    /// List secrets.
    List,

    /// Get a secret value.
    Get {
        /// Secret key.
        key: String,
    },

    /// Set a secret value.
    Set {
        /// Secret key.
        key: String,

        /// Secret value (or @file.json to read from file, or - for stdin).
        value: String,
    },

    /// Delete a secret.
    Unset {
        /// Secret key.
        key: String,
    },

    /// Rotate a transit encryption key.
    Rotate {
        /// Transit key name.
        key_name: String,
    },
}

#[derive(Debug, Parser)]
pub struct CloudArgs {
    #[command(subcommand)]
    pub command: CloudCommands,
}

#[derive(Debug, Subcommand)]
pub enum CloudCommands {
    /// Manage projects.
    Projects(CloudProjectsArgs),

    /// Manage API keys.
    Keys(CloudKeysArgs),

    /// Manage project members.
    Members(CloudMembersArgs),

    /// Manage DNS domains on reactor.cloud (Cloudflare).
    Domains(CloudDomainsArgs),

    /// View audit log.
    Audit {
        /// Project ref.
        project_ref: String,

        /// Maximum entries to show.
        #[arg(long, default_value = "50")]
        limit: i32,
    },
}

#[derive(Debug, Parser)]
pub struct CloudProjectsArgs {
    #[command(subcommand)]
    pub command: CloudProjectsCommands,
}

#[derive(Debug, Subcommand)]
pub enum CloudProjectsCommands {
    /// Create a new project.
    Create {
        /// Project name.
        name: String,

        /// Deployment region.
        #[arg(long)]
        region: Option<String>,

        /// Provision a reactor.cloud subdomain (Cloudflare CNAME -> edge).
        /// Pass a bare label (e.g., "antennanew") or FQDN ("antennanew.reactor.cloud").
        #[arg(long)]
        subdomain: Option<String>,

        /// Cloudflare API token (overrides CF_API_TOKEN env var).
        #[arg(long)]
        cf_token: Option<String>,
    },

    /// List projects.
    List {
        /// Filter by owner user ID.
        #[arg(long)]
        owner: Option<uuid::Uuid>,

        /// Maximum results.
        #[arg(long, default_value = "50")]
        limit: i32,

        /// Offset for pagination.
        #[arg(long, default_value = "0")]
        offset: i32,
    },

    /// Show project details.
    Show {
        /// Project ref.
        project_ref: String,
    },

    /// Delete a project.
    Delete {
        /// Project ref.
        project_ref: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Parser)]
pub struct CloudKeysArgs {
    #[command(subcommand)]
    pub command: CloudKeysCommands,
}

#[derive(Debug, Subcommand)]
pub enum CloudKeysCommands {
    /// List API keys.
    List {
        /// Project ref.
        project_ref: String,
    },

    /// Create a new API key.
    Create {
        /// Project ref.
        project_ref: String,

        /// Key kind (anon or service).
        #[arg(long)]
        kind: String,
    },

    /// Rotate an API key.
    Rotate {
        /// Project ref.
        project_ref: String,

        /// Key ID to rotate.
        key_id: String,
    },

    /// Revoke an API key.
    Revoke {
        /// Project ref.
        project_ref: String,

        /// Key ID to revoke.
        key_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct CloudMembersArgs {
    #[command(subcommand)]
    pub command: CloudMembersCommands,
}

#[derive(Debug, Subcommand)]
pub enum CloudMembersCommands {
    /// List project members.
    List {
        /// Project ref.
        project_ref: String,
    },

    /// Add a member to a project.
    Add {
        /// Project ref.
        project_ref: String,

        /// User ID to add.
        user_id: String,

        /// Member role (admin or member).
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Remove a member from a project.
    Remove {
        /// Project ref.
        project_ref: String,

        /// User ID to remove.
        user_id: String,
    },
}

#[derive(Debug, Parser)]
pub struct CloudDomainsArgs {
    #[command(subcommand)]
    pub command: CloudDomainsCommands,

    /// Cloudflare API token (overrides CF_API_TOKEN env var).
    #[arg(long, global = true)]
    pub cf_token: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum CloudDomainsCommands {
    /// Add a subdomain to reactor.cloud (creates Cloudflare CNAME).
    Add {
        /// Subdomain label (e.g., "antennanew") or FQDN ("antennanew.reactor.cloud").
        name: String,

        /// Target hostname for the CNAME record.
        #[arg(long, default_value = "rc-shared-1-edge.fly.dev")]
        target: String,
    },

    /// List reactor.cloud subdomains.
    List,

    /// Remove a subdomain from reactor.cloud.
    Remove {
        /// Subdomain label (e.g., "antennanew") or FQDN ("antennanew.reactor.cloud").
        name: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}

// ============================================================================
// Connect command arguments
// ============================================================================

#[derive(Debug, Parser)]
pub struct ConnectArgs {
    #[command(subcommand)]
    pub command: ConnectCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConnectCommands {
    /// List available connectors in the catalog.
    Catalog(ConnectCatalogArgs),

    /// Manage connector instances.
    Instances(ConnectInstancesArgs),

    /// Invoke an action on a connector instance.
    Action(ConnectActionArgs),

    /// Manage webhook receivers.
    Receivers(ConnectReceiversArgs),

    /// Manage schema drift events.
    Drift(ConnectDriftArgs),

    /// Generate TypeScript SDK types for a connector instance.
    Codegen(ConnectCodegenArgs),

    /// Authenticate with an OAuth connector.
    #[cfg(feature = "connect-oauth")]
    Auth(ConnectAuthArgs),
}

#[derive(Debug, Parser)]
pub struct ConnectCodegenArgs {
    /// Instance name to generate types for.
    #[arg(long)]
    pub instance: String,

    /// Output directory (default: ./generated).
    #[arg(long, default_value = "./generated")]
    pub output: std::path::PathBuf,

    /// Output format (typescript, json-schema).
    #[arg(long, default_value = "typescript")]
    pub format: String,
}

#[derive(Debug, Parser)]
pub struct ConnectCatalogArgs {
    #[command(subcommand)]
    pub command: ConnectCatalogCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConnectCatalogCommands {
    /// List available connectors.
    List,

    /// Show connector details.
    Show {
        /// Connector type ID (e.g., "stripe", "slack").
        connector_type: String,
    },
}

#[derive(Debug, Parser)]
pub struct ConnectInstancesArgs {
    #[command(subcommand)]
    pub command: ConnectInstancesCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConnectInstancesCommands {
    /// List connector instances.
    List,

    /// Create a new connector instance.
    Create {
        /// Connector type ID (e.g., "stripe", "slack").
        connector_type: String,

        /// Instance name.
        #[arg(long)]
        name: String,

        /// Configuration JSON (or @file.json).
        #[arg(long)]
        config: Option<String>,
    },

    /// Show instance details.
    Show {
        /// Instance ID.
        instance_id: String,
    },

    /// Update instance configuration.
    Update {
        /// Instance ID.
        instance_id: String,

        /// New name.
        #[arg(long)]
        name: Option<String>,

        /// Configuration JSON (or @file.json).
        #[arg(long)]
        config: Option<String>,
    },

    /// Delete a connector instance.
    Delete {
        /// Instance ID.
        instance_id: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Test instance credentials.
    Check {
        /// Instance ID.
        instance_id: String,
    },

    /// Set credentials for an instance.
    Credentials {
        /// Instance ID.
        instance_id: String,

        /// Credentials JSON (or @file.json, or - for stdin).
        credentials: String,
    },
}

#[derive(Debug, Parser)]
pub struct ConnectActionArgs {
    /// Instance ID.
    pub instance_id: String,

    /// Action name (e.g., "createCustomer", "postMessage").
    pub action: String,

    /// Action input JSON (or @file.json, or - for stdin).
    #[arg(long)]
    pub input: Option<String>,

    /// Run in dry-run/sandbox mode.
    #[arg(long)]
    pub dry_run: bool,

    /// Idempotency key for the action.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Parser)]
pub struct ConnectReceiversArgs {
    #[command(subcommand)]
    pub command: ConnectReceiversCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConnectReceiversCommands {
    /// List webhook receivers.
    List,

    /// Create a new receiver.
    Create {
        /// Receiver name.
        #[arg(long)]
        name: String,

        /// Target type (job, stream, action, function).
        #[arg(long)]
        target_type: String,

        /// Target name or connection ID.
        #[arg(long)]
        target: String,

        /// Filter expression (optional).
        #[arg(long)]
        filter: Option<String>,
    },

    /// Show receiver details.
    Show {
        /// Receiver ID.
        receiver_id: String,
    },

    /// Delete a receiver.
    Delete {
        /// Receiver ID.
        receiver_id: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Rotate receiver token.
    Rotate {
        /// Receiver ID.
        receiver_id: String,

        /// Grace period in seconds for old token.
        #[arg(long, default_value = "300")]
        grace_seconds: u64,
    },
}

#[cfg(feature = "connect-oauth")]
#[derive(Debug, Parser)]
pub struct ConnectAuthArgs {
    /// Instance ID to authenticate.
    pub instance_id: String,

    /// Skip opening browser, print URL instead.
    #[arg(long)]
    pub no_browser: bool,

    /// Port for OAuth callback server.
    #[arg(long, default_value = "9876")]
    pub port: u16,
}

#[derive(Debug, Parser)]
pub struct ConnectDriftArgs {
    #[command(subcommand)]
    pub command: ConnectDriftCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConnectDriftCommands {
    /// List pending schema drift events.
    List {
        /// Filter by connection name.
        #[arg(long)]
        connection: Option<String>,

        /// Include all statuses (not just pending).
        #[arg(long)]
        all: bool,
    },

    /// Approve a schema drift event.
    Approve {
        /// Drift event ID.
        drift_id: String,

        /// Reason for approval.
        #[arg(long)]
        reason: Option<String>,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Reject a schema drift event.
    Reject {
        /// Drift event ID.
        drift_id: String,

        /// Reason for rejection.
        #[arg(long)]
        reason: Option<String>,

        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}
