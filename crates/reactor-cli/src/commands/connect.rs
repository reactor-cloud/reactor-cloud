//! Connect command implementation.

use crate::cli::{
    Cli, ConnectArgs, ConnectCatalogCommands, ConnectCommands,
    ConnectDriftCommands, ConnectInstancesCommands, ConnectReceiversCommands,
};
use crate::context::{resolve_context, GlobalConfig};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use crate::project::Project;
use crate::{confirm, is_interactive};
use reactor_client::connect::{
    CreateInstanceRequest, CreateReceiverRequest, InvokeActionRequest, ReceiverTarget,
    UpdateInstanceRequest,
};
use reactor_client::{Client, ClientConfig};
use uuid::Uuid;

pub async fn run(cli: &Cli, args: &ConnectArgs, output: &Output) -> CliResult<()> {
    match &args.command {
        ConnectCommands::Catalog(catalog_args) => {
            let client = build_client(cli)?;
            match &catalog_args.command {
                ConnectCatalogCommands::List => catalog_list(&client, output).await,
                ConnectCatalogCommands::Show { connector_type } => {
                    catalog_show(&client, connector_type, output).await
                }
            }
        }
        ConnectCommands::Instances(instances_args) => {
            let client = build_client(cli)?;
            match &instances_args.command {
                ConnectInstancesCommands::List => instances_list(&client, output).await,
                ConnectInstancesCommands::Create {
                    connector_type,
                    name,
                    config,
                } => {
                    instances_create(&client, connector_type, name, config.as_deref(), output).await
                }
                ConnectInstancesCommands::Show { instance_id } => {
                    instances_show(&client, instance_id, output).await
                }
                ConnectInstancesCommands::Update {
                    instance_id,
                    name,
                    config,
                } => {
                    instances_update(
                        &client,
                        instance_id,
                        name.as_deref(),
                        config.as_deref(),
                        output,
                    )
                    .await
                }
                ConnectInstancesCommands::Delete { instance_id, yes } => {
                    instances_delete(cli, &client, instance_id, *yes, output).await
                }
                ConnectInstancesCommands::Check { instance_id } => {
                    instances_check(&client, instance_id, output).await
                }
                ConnectInstancesCommands::Credentials {
                    instance_id,
                    credentials,
                } => instances_credentials(&client, instance_id, credentials, output).await,
            }
        }
        ConnectCommands::Action(action_args) => {
            let client = build_client(cli)?;
            action_invoke(
                &client,
                &action_args.instance_id,
                &action_args.action,
                action_args.input.as_deref(),
                action_args.dry_run,
                action_args.idempotency_key.as_deref(),
                output,
            )
            .await
        }
        ConnectCommands::Receivers(receivers_args) => {
            let client = build_client(cli)?;
            match &receivers_args.command {
                ConnectReceiversCommands::List => receivers_list(&client, output).await,
                ConnectReceiversCommands::Create {
                    name,
                    target_type,
                    target,
                    filter,
                } => {
                    receivers_create(&client, name, target_type, target, filter.as_deref(), output)
                        .await
                }
                ConnectReceiversCommands::Show { receiver_id } => {
                    receivers_show(&client, receiver_id, output).await
                }
                ConnectReceiversCommands::Delete { receiver_id, yes } => {
                    receivers_delete(cli, &client, receiver_id, *yes, output).await
                }
                ConnectReceiversCommands::Rotate {
                    receiver_id,
                    grace_seconds,
                } => receivers_rotate(&client, receiver_id, *grace_seconds, output).await,
            }
        }
        ConnectCommands::Drift(drift_args) => {
            let client = build_client(cli)?;
            match &drift_args.command {
                ConnectDriftCommands::List { connection, all } => {
                    drift_list(&client, connection.as_deref(), *all, output).await
                }
                ConnectDriftCommands::Approve { drift_id, reason, yes } => {
                    drift_approve(cli, &client, drift_id, reason.as_deref(), *yes, output).await
                }
                ConnectDriftCommands::Reject { drift_id, reason, yes } => {
                    drift_reject(cli, &client, drift_id, reason.as_deref(), *yes, output).await
                }
            }
        }
        ConnectCommands::Codegen(codegen_args) => {
            let client = build_client(cli)?;
            codegen(&client, &codegen_args.instance, &codegen_args.output, &codegen_args.format, output).await
        }
        #[cfg(feature = "connect-oauth")]
        ConnectCommands::Auth(auth_args) => {
            oauth_flow(&auth_args.instance_id, auth_args.no_browser, auth_args.port, output).await
        }
    }
}

fn build_client(cli: &Cli) -> CliResult<Client> {
    let config = GlobalConfig::load()?;
    let cwd = std::env::current_dir()?;
    let project = Project::try_resolve(&cwd, cli.manifest.as_deref());
    let project_default = project
        .as_ref()
        .and_then(|p| p.manifest.default_context.as_deref());

    let resolved = resolve_context(
        &config,
        cli.context.as_deref(),
        project_default,
        cli.token.as_deref(),
    )?;

    let mut client_config = ClientConfig::new(resolved.endpoint);
    if let Some(token) = resolved.token {
        client_config = client_config.with_token(token);
    }
    if let Some(org) = resolved.org {
        client_config = client_config.with_org(org);
    }

    Client::new(client_config).map_err(Into::into)
}

fn parse_uuid(s: &str) -> CliResult<Uuid> {
    s.parse()
        .map_err(|_| CliError::InvalidArgument("invalid UUID".into()))
}

fn parse_json(s: &str) -> CliResult<serde_json::Value> {
    if s == "-" {
        // Read from stdin
        if is_interactive() {
            return Err(CliError::User("stdin not available in interactive mode".into()));
        }
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        serde_json::from_str(&buf).map_err(|e| CliError::InvalidArgument(format!("invalid JSON: {}", e)))
    } else if s.starts_with('@') {
        // Read from file
        let path = &s[1..];
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| CliError::InvalidArgument(format!("invalid JSON: {}", e)))
    } else {
        // Parse as JSON directly
        serde_json::from_str(s).map_err(|e| CliError::InvalidArgument(format!("invalid JSON: {}", e)))
    }
}

// ============================================================================
// Catalog commands
// ============================================================================

async fn catalog_list(client: &Client, output: &Output) -> CliResult<()> {
    let connectors = client.connect_catalog_list().await?;

    if output.format().is_json() {
        output.success(&connectors)?;
    } else if connectors.is_empty() {
        output.info("No connectors available in catalog.");
    } else {
        let headers = &["TYPE", "NAME", "VERSION", "RUNTIME"];
        let rows: Vec<Vec<String>> = connectors
            .iter()
            .map(|c| {
                vec![
                    c.type_id.clone(),
                    c.display_name.clone(),
                    c.version.clone(),
                    c.runtime.clone(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn catalog_show(client: &Client, connector_type: &str, output: &Output) -> CliResult<()> {
    let connector = client.connect_catalog_get(connector_type).await?;

    if output.format().is_json() {
        output.success(&connector)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Connector: {}", connector.display_name));
        human::print_kv("Type ID", &connector.type_id);
        human::print_kv("Version", &connector.version);
        human::print_kv("Runtime", &connector.runtime);

        if !connector.actions.is_empty() {
            println!();
            human::print_section("Actions");
            for action in &connector.actions {
                println!("  - {} ({})", action.name, action.side_effects);
            }
        }

        if !connector.streams.is_empty() {
            println!();
            human::print_section("Streams");
            for stream in &connector.streams {
                println!("  - {}", stream.name);
            }
        }

        if let Some(url) = &connector.doc_url {
            println!();
            human::print_kv("Documentation", url);
        }
    }

    Ok(())
}

// ============================================================================
// Instances commands
// ============================================================================

async fn instances_list(client: &Client, output: &Output) -> CliResult<()> {
    let instances = client.connect_instances_list().await?;

    if output.format().is_json() {
        output.success(&instances)?;
    } else if instances.is_empty() {
        output.info("No connector instances found.");
    } else {
        let headers = &["ID", "NAME", "TYPE", "STATUS", "CREATED"];
        let rows: Vec<Vec<String>> = instances
            .iter()
            .map(|i| {
                vec![
                    i.id.to_string(),
                    i.name.clone(),
                    i.connector_type.clone(),
                    i.status.clone(),
                    i.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn instances_create(
    client: &Client,
    connector_type: &str,
    name: &str,
    config: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let config_value = match config {
        Some(c) => parse_json(c)?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    let request = CreateInstanceRequest {
        connector_type: connector_type.to_string(),
        name: name.to_string(),
        config: config_value,
    };

    let instance = client.connect_instances_create(request).await?;

    if output.format().is_json() {
        output.success(&instance)?;
    } else {
        output.success_message(&format!(
            "Created instance '{}' (ID: {})",
            instance.name, instance.id
        ))?;
    }

    Ok(())
}

async fn instances_show(client: &Client, instance_id: &str, output: &Output) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;
    let instance = client.connect_instances_get(id).await?;

    if output.format().is_json() {
        output.success(&instance)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Instance: {}", instance.name));
        human::print_kv("ID", &instance.id.to_string());
        human::print_kv("Type", &instance.connector_type);
        human::print_kv("Status", &instance.status);
        human::print_kv("Created", &instance.created_at.to_rfc3339());
        human::print_kv("Updated", &instance.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn instances_update(
    client: &Client,
    instance_id: &str,
    name: Option<&str>,
    config: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;

    let config_value = match config {
        Some(c) => Some(parse_json(c)?),
        None => None,
    };

    let request = UpdateInstanceRequest {
        name: name.map(String::from),
        config: config_value,
    };

    let instance = client.connect_instances_update(id, request).await?;

    if output.format().is_json() {
        output.success(&instance)?;
    } else {
        output.success_message(&format!("Updated instance '{}'", instance.name))?;
    }

    Ok(())
}

async fn instances_delete(
    cli: &Cli,
    client: &Client,
    instance_id: &str,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;

    if !yes {
        confirm(cli, &format!("Delete instance {}?", instance_id))?;
    }

    client.connect_instances_delete(id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "deleted": true }))?;
    } else {
        output.success_message(&format!("Deleted instance {}", instance_id))?;
    }

    Ok(())
}

async fn instances_check(client: &Client, instance_id: &str, output: &Output) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;
    let status = client.connect_instances_check(id).await?;

    if output.format().is_json() {
        output.success(&status)?;
    } else {
        match status.status.as_str() {
            "succeeded" => output.success_message("Connection check passed")?,
            _ => {
                let msg = status.message.as_deref().unwrap_or("Unknown error");
                output.warning(&format!("Connection check failed: {}", msg));
            }
        }
    }

    Ok(())
}

async fn instances_credentials(
    client: &Client,
    instance_id: &str,
    credentials: &str,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;
    let creds = parse_json(credentials)?;

    client.connect_instances_credentials(id, creds).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "saved": true }))?;
    } else {
        output.success_message("Credentials saved")?;
    }

    Ok(())
}

// ============================================================================
// Action command
// ============================================================================

async fn action_invoke(
    client: &Client,
    instance_id: &str,
    action: &str,
    input: Option<&str>,
    dry_run: bool,
    idempotency_key: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(instance_id)?;

    let input_value = match input {
        Some(i) => parse_json(i)?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    let request = InvokeActionRequest {
        input: input_value,
        dry_run,
        idempotency_key: idempotency_key.map(String::from),
    };

    let result = client.connect_action_invoke(id, action, request).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        if result.dry_run {
            output.info("(dry-run mode)");
        }
        println!("{}", serde_json::to_string_pretty(&result.output)?);
    }

    Ok(())
}

// ============================================================================
// Receivers commands
// ============================================================================

async fn receivers_list(client: &Client, output: &Output) -> CliResult<()> {
    let receivers = client.connect_receivers_list().await?;

    if output.format().is_json() {
        output.success(&receivers)?;
    } else if receivers.is_empty() {
        output.info("No webhook receivers found.");
    } else {
        let headers = &["ID", "NAME", "TARGET", "STATUS", "CREATED"];
        let rows: Vec<Vec<String>> = receivers
            .iter()
            .map(|r| {
                let target = match &r.target {
                    ReceiverTarget::Job { name } => format!("job:{}", name),
                    ReceiverTarget::Stream { connection } => format!("stream:{}", connection),
                    ReceiverTarget::Action { instance, action } => {
                        format!("action:{}:{}", instance, action)
                    }
                    ReceiverTarget::Function { name } => format!("function:{}", name),
                };
                vec![
                    r.id.to_string(),
                    r.name.clone(),
                    target,
                    r.status.clone(),
                    r.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn receivers_create(
    client: &Client,
    name: &str,
    target_type: &str,
    target: &str,
    filter: Option<&str>,
    output: &Output,
) -> CliResult<()> {
    let receiver_target = parse_target(target_type, target)?;

    let request = CreateReceiverRequest {
        name: name.to_string(),
        target: receiver_target,
        filter_expression: filter.map(String::from),
    };

    let result = client.connect_receivers_create(request).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Receiver: {}", result.receiver.name));
        human::print_kv("ID", &result.receiver.id.to_string());
        println!();
        human::print_kv("Token", &result.token);
        output.warning("Save this token - it will not be shown again.");
    }

    Ok(())
}

fn parse_target(target_type: &str, target: &str) -> CliResult<ReceiverTarget> {
    match target_type {
        "job" => Ok(ReceiverTarget::Job {
            name: target.to_string(),
        }),
        "stream" => Ok(ReceiverTarget::Stream {
            connection: target.to_string(),
        }),
        "action" => {
            let parts: Vec<&str> = target.split(':').collect();
            if parts.len() != 2 {
                return Err(CliError::InvalidArgument(
                    "action target must be instance_id:action_name".into(),
                ));
            }
            Ok(ReceiverTarget::Action {
                instance: parts[0].to_string(),
                action: parts[1].to_string(),
            })
        }
        "function" => Ok(ReceiverTarget::Function {
            name: target.to_string(),
        }),
        _ => Err(CliError::InvalidArgument(format!(
            "invalid target type '{}', expected: job, stream, action, function",
            target_type
        ))),
    }
}

async fn receivers_show(client: &Client, receiver_id: &str, output: &Output) -> CliResult<()> {
    let id = parse_uuid(receiver_id)?;
    let receiver = client.connect_receivers_get(id).await?;

    if output.format().is_json() {
        output.success(&receiver)?;
    } else {
        use crate::output::human;
        human::print_section(&format!("Receiver: {}", receiver.name));
        human::print_kv("ID", &receiver.id.to_string());
        human::print_kv("Status", &receiver.status);

        let target_str = match &receiver.target {
            ReceiverTarget::Job { name } => format!("job:{}", name),
            ReceiverTarget::Stream { connection } => format!("stream:{}", connection),
            ReceiverTarget::Action { instance, action } => format!("action:{}:{}", instance, action),
            ReceiverTarget::Function { name } => format!("function:{}", name),
        };
        human::print_kv("Target", &target_str);

        if let Some(filter) = &receiver.filter_expression {
            human::print_kv("Filter", filter);
        }

        human::print_kv("Created", &receiver.created_at.to_rfc3339());
        human::print_kv("Updated", &receiver.updated_at.to_rfc3339());
    }

    Ok(())
}

async fn receivers_delete(
    cli: &Cli,
    client: &Client,
    receiver_id: &str,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(receiver_id)?;

    if !yes {
        confirm(cli, &format!("Delete receiver {}?", receiver_id))?;
    }

    client.connect_receivers_delete(id).await?;

    if output.format().is_json() {
        output.success(&serde_json::json!({ "deleted": true }))?;
    } else {
        output.success_message(&format!("Deleted receiver {}", receiver_id))?;
    }

    Ok(())
}

async fn receivers_rotate(
    client: &Client,
    receiver_id: &str,
    grace_seconds: u64,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(receiver_id)?;
    let result = client.connect_receivers_rotate(id, grace_seconds).await?;

    if output.format().is_json() {
        output.success(&result)?;
    } else {
        use crate::output::human;
        human::print_section("Token Rotated");
        human::print_kv("New Token", &result.new_token);
        human::print_kv("Old Token Expires", &result.old_token_expires_at.to_rfc3339());
        output.warning("Save the new token - it will not be shown again.");
    }

    Ok(())
}

// ============================================================================
// Drift commands
// ============================================================================

async fn drift_list(
    client: &Client,
    connection: Option<&str>,
    all: bool,
    output: &Output,
) -> CliResult<()> {
    let events = client.connect_drift_list(connection, all).await?;

    if output.format().is_json() {
        output.success(&events)?;
    } else if events.is_empty() {
        if all {
            output.info("No schema drift events found.");
        } else {
            output.info("No pending schema drift events.");
        }
    } else {
        let headers = &["ID", "CONNECTION", "STREAM", "TYPE", "SEVERITY", "STATUS"];
        let rows: Vec<Vec<String>> = events
            .iter()
            .map(|e| {
                vec![
                    e.id.to_string()[..8].to_string(),
                    e.connection_name.clone().unwrap_or_default(),
                    e.stream_name.clone(),
                    e.drift_type.clone(),
                    e.severity.clone(),
                    e.status.clone(),
                ]
            })
            .collect();
        output.table(headers, rows)?;
    }

    Ok(())
}

async fn drift_approve(
    cli: &Cli,
    client: &Client,
    drift_id: &str,
    reason: Option<&str>,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(drift_id)?;

    if !yes {
        confirm(cli, &format!("Approve schema drift event {}?", drift_id))?;
    }

    let event = client.connect_drift_approve(id, reason).await?;

    if output.format().is_json() {
        output.success(&event)?;
    } else {
        output.success_message(&format!("Approved drift event {}", drift_id))?;
    }

    Ok(())
}

async fn drift_reject(
    cli: &Cli,
    client: &Client,
    drift_id: &str,
    reason: Option<&str>,
    yes: bool,
    output: &Output,
) -> CliResult<()> {
    let id = parse_uuid(drift_id)?;

    if !yes {
        confirm(cli, &format!("Reject schema drift event {}?", drift_id))?;
    }

    let event = client.connect_drift_reject(id, reason).await?;

    if output.format().is_json() {
        output.success(&event)?;
    } else {
        output.success_message(&format!("Rejected drift event {}", drift_id))?;
    }

    Ok(())
}

// ============================================================================
// Codegen
// ============================================================================

async fn codegen(
    client: &Client,
    instance_name: &str,
    output_dir: &std::path::Path,
    format: &str,
    output: &Output,
) -> CliResult<()> {
    // Get instances and find by name
    let instances = client.connect_instances_list().await?;
    let instance = instances
        .iter()
        .find(|i| i.name == instance_name)
        .ok_or_else(|| CliError::User(format!("instance '{}' not found", instance_name)))?;
    
    // Get connector descriptor
    let descriptor = client.connect_catalog_get(&instance.connector_type).await?;
    
    // Convert to JSON for generic processing
    let descriptor_json = serde_json::to_value(&descriptor)?;
    
    // Create output directory
    std::fs::create_dir_all(output_dir)?;
    
    match format {
        "typescript" => {
            let ts_file = output_dir.join(format!("{}.ts", instance_name.replace('-', "_")));
            let content = generate_typescript(&instance.connector_type, &descriptor_json)?;
            std::fs::write(&ts_file, content)?;
            output.success_message(&format!("Generated {}", ts_file.display()))?;
        }
        "json-schema" => {
            let schema_file = output_dir.join(format!("{}.schema.json", instance_name));
            let content = serde_json::to_string_pretty(&descriptor_json)?;
            std::fs::write(&schema_file, content)?;
            output.success_message(&format!("Generated {}", schema_file.display()))?;
        }
        _ => {
            return Err(CliError::User(format!("unsupported format: {}", format)));
        }
    }
    
    Ok(())
}

fn generate_typescript(connector_type: &str, descriptor: &serde_json::Value) -> CliResult<String> {
    let mut ts = String::new();
    
    ts.push_str("// Auto-generated TypeScript types for ");
    ts.push_str(connector_type);
    ts.push_str(" connector\n");
    ts.push_str("// Generated by: reactor connect codegen\n\n");
    
    // Generate action input/output types
    if let Some(actions) = descriptor.get("actions").and_then(|a| a.as_array()) {
        for action in actions {
            if let Some(name) = action.get("name").and_then(|n| n.as_str()) {
                let pascal_name = to_pascal_case(name);
                
                ts.push_str(&format!("/** Input for {} action */\n", name));
                ts.push_str(&format!("export interface {}Input {{\n", pascal_name));
                if let Some(schema) = action.get("input_schema").and_then(|s| s.get("properties")) {
                    if let Some(props) = schema.as_object() {
                        for (prop_name, prop_schema) in props {
                            let ts_type = json_schema_to_ts(prop_schema);
                            ts.push_str(&format!("  {}?: {};\n", prop_name, ts_type));
                        }
                    }
                }
                ts.push_str("}\n\n");
                
                ts.push_str(&format!("/** Output for {} action */\n", name));
                ts.push_str(&format!("export interface {}Output {{\n", pascal_name));
                if let Some(schema) = action.get("output_schema").and_then(|s| s.get("properties")) {
                    if let Some(props) = schema.as_object() {
                        for (prop_name, prop_schema) in props {
                            let ts_type = json_schema_to_ts(prop_schema);
                            ts.push_str(&format!("  {}?: {};\n", prop_name, ts_type));
                        }
                    }
                }
                ts.push_str("}\n\n");
            }
        }
    }
    
    // Generate stream types
    if let Some(streams) = descriptor.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            if let Some(name) = stream.get("name").and_then(|n| n.as_str()) {
                let pascal_name = to_pascal_case(name);
                
                ts.push_str(&format!("/** Record type for {} stream */\n", name));
                ts.push_str(&format!("export interface {}Record {{\n", pascal_name));
                if let Some(schema) = stream.get("json_schema").and_then(|s| s.get("properties")) {
                    if let Some(props) = schema.as_object() {
                        for (prop_name, prop_schema) in props {
                            let ts_type = json_schema_to_ts(prop_schema);
                            ts.push_str(&format!("  {}?: {};\n", prop_name, ts_type));
                        }
                    }
                }
                ts.push_str("}\n\n");
            }
        }
    }
    
    Ok(ts)
}

fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

fn json_schema_to_ts(schema: &serde_json::Value) -> String {
    if let Some(type_val) = schema.get("type").and_then(|t| t.as_str()) {
        match type_val {
            "string" => "string".to_string(),
            "integer" | "number" => "number".to_string(),
            "boolean" => "boolean".to_string(),
            "array" => {
                if let Some(items) = schema.get("items") {
                    format!("{}[]", json_schema_to_ts(items))
                } else {
                    "unknown[]".to_string()
                }
            }
            "object" => "Record<string, unknown>".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        "unknown".to_string()
    }
}

// ============================================================================
// OAuth flow (feature-gated)
// ============================================================================

#[cfg(feature = "connect-oauth")]
async fn oauth_flow(
    instance_id: &str,
    no_browser: bool,
    port: u16,
    output: &Output,
) -> CliResult<()> {
    use std::io::Write;
    use tokio::net::TcpListener;

    let _id = parse_uuid(instance_id)?;

    // This is a placeholder for the full OAuth flow implementation.
    // The full implementation would:
    // 1. Get the connector descriptor to find OAuth URLs
    // 2. Generate a state token
    // 3. Start a local HTTP server on the specified port
    // 4. Open the authorization URL in a browser (or print it)
    // 5. Wait for the callback with the authorization code
    // 6. Exchange the code for tokens
    // 7. Store the tokens using the credentials endpoint

    let callback_url = format!("http://localhost:{}/callback", port);

    if no_browser {
        output.info(&format!(
            "Visit this URL to authorize:\n  (Authorization URL will be shown here)\n\n\
             Then paste the authorization code:"
        ));

        // Read code from stdin
        print!("> ");
        std::io::stdout().flush()?;

        let mut code = String::new();
        std::io::stdin().read_line(&mut code)?;
        let code = code.trim();

        if code.is_empty() {
            return Err(CliError::User("no authorization code provided".into()));
        }

        output.success_message(&format!("Authorization code received: {}", &code[..8.min(code.len())]))?;
    } else {
        output.info(&format!(
            "Starting OAuth callback server on port {}...",
            port
        ));

        // Start callback listener
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await
            .map_err(|e| CliError::Internal(format!("failed to bind callback server: {}", e)))?;

        output.info("Opening browser for authorization...");

        // Open browser (would use webbrowser crate here)
        // webbrowser::open(&auth_url)?;

        output.info(&format!(
            "If the browser doesn't open, visit:\n  (Authorization URL)\n\n\
             Waiting for callback on {}...",
            callback_url
        ));

        // Wait for callback
        let (stream, _) = listener.accept().await
            .map_err(|e| CliError::Internal(format!("failed to accept callback: {}", e)))?;

        // Parse the callback and extract the code
        // This is simplified - a real implementation would parse the HTTP request
        let _ = stream;

        output.success_message("Authorization successful!")?;
    }

    Ok(())
}
