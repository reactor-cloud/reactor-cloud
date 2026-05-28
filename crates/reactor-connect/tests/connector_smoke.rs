//! Smoke tests for native connectors.
//!
//! These tests verify basic connector functionality without requiring
//! actual API credentials - they test descriptor validity and dry-run modes.

use reactor_connect::connectors::{GitHubConnector, LinearConnector, SlackConnector, StripeConnector};
use reactor_connect::runtime::native::NativeConnector;
use reactor_connect::runtime::ActionOpts;

// =============================================================================
// Descriptor Tests - Verify connector descriptors are well-formed
// =============================================================================

#[test]
fn test_stripe_descriptor_valid() {
    let connector = StripeConnector::new();
    let descriptor = connector.descriptor();

    assert_eq!(descriptor.type_id, "stripe");
    assert_eq!(descriptor.display_name, "Stripe");
    assert!(!descriptor.actions.is_empty());

    // Verify createCustomer action exists
    let create_customer = descriptor
        .actions
        .iter()
        .find(|a| a.name == "createCustomer")
        .expect("createCustomer action should exist");

    assert_eq!(
        create_customer.side_effects,
        reactor_connect::descriptor::SideEffectKind::Mutates
    );
}

#[test]
fn test_slack_descriptor_valid() {
    let connector = SlackConnector::new();
    let descriptor = connector.descriptor();

    assert_eq!(descriptor.type_id, "slack");
    assert_eq!(descriptor.display_name, "Slack");
    assert!(!descriptor.actions.is_empty());

    // Verify postMessage action exists
    let post_message = descriptor
        .actions
        .iter()
        .find(|a| a.name == "postMessage")
        .expect("postMessage action should exist");

    assert_eq!(
        post_message.side_effects,
        reactor_connect::descriptor::SideEffectKind::Sends
    );
}

#[test]
fn test_linear_descriptor_valid() {
    let connector = LinearConnector::new();
    let descriptor = connector.descriptor();

    assert_eq!(descriptor.type_id, "linear");
    assert_eq!(descriptor.display_name, "Linear");
    assert!(!descriptor.actions.is_empty());

    // Verify createIssue action exists
    let create_issue = descriptor
        .actions
        .iter()
        .find(|a| a.name == "createIssue")
        .expect("createIssue action should exist");

    assert_eq!(
        create_issue.side_effects,
        reactor_connect::descriptor::SideEffectKind::Mutates
    );
}

#[test]
fn test_github_descriptor_valid() {
    let connector = GitHubConnector::new();
    let descriptor = connector.descriptor();

    assert_eq!(descriptor.type_id, "github");
    assert_eq!(descriptor.display_name, "GitHub");
    assert!(!descriptor.actions.is_empty());
    assert!(!descriptor.streams.is_empty());

    // Verify issues stream exists
    let issues_stream = descriptor
        .streams
        .iter()
        .find(|s| s.name == "issues")
        .expect("issues stream should exist");

    assert!(!issues_stream.supported_modes.is_empty());
}

// =============================================================================
// Dry-Run Tests - Verify dry-run mode produces valid responses
// =============================================================================

#[tokio::test]
async fn test_stripe_create_customer_dry_run() {
    let connector = StripeConnector::new();

    // Use a fake live key to force synthesized dry-run
    let config = serde_json::json!({
        "api_key": "sk_live_fake_key_for_testing"
    });

    let input = serde_json::json!({
        "email": "test@example.com",
        "name": "Test User"
    });

    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "createCustomer", &input, &opts)
        .await
        .unwrap();

    // Should be a synthesized dry-run response
    assert!(result.get("_dry_run").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(result.get("id").is_some());
    assert_eq!(result.get("email"), Some(&serde_json::json!("test@example.com")));
}

#[tokio::test]
async fn test_slack_post_message_dry_run() {
    let connector = SlackConnector::new();

    let config = serde_json::json!({
        "access_token": "xoxb-fake-token"
    });

    let input = serde_json::json!({
        "channel": "C12345678",
        "text": "Test message"
    });

    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "postMessage", &input, &opts)
        .await
        .unwrap();

    // Should be a synthesized dry-run response
    assert!(result.get("_dry_run").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(result.get("ok"), Some(&serde_json::json!(true)));
    assert!(result.get("ts").is_some());
}

#[tokio::test]
async fn test_linear_create_issue_dry_run() {
    let connector = LinearConnector::new();

    let config = serde_json::json!({
        "api_key": "lin_api_fake_key"
    });

    let input = serde_json::json!({
        "title": "Test Issue",
        "teamId": "team_123"
    });

    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "createIssue", &input, &opts)
        .await
        .unwrap();

    // Should be a synthesized dry-run response
    let issue_create = result.get("issueCreate").expect("issueCreate should exist");
    assert!(
        issue_create
            .get("issue")
            .and_then(|i| i.get("_dry_run"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    );
}

#[tokio::test]
async fn test_github_create_issue_dry_run() {
    let connector = GitHubConnector::new();

    let config = serde_json::json!({
        "access_token": "ghp_fake_token",
        "owner": "test-org",
        "repo": "test-repo"
    });

    let input = serde_json::json!({
        "title": "Test Issue",
        "body": "Test body"
    });

    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "createIssue", &input, &opts)
        .await
        .unwrap();

    // Should be a synthesized dry-run response
    assert!(result.get("_dry_run").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(result.get("html_url").is_some());
}

// =============================================================================
// Auth Validation Tests - Verify config validation without actual API calls
// =============================================================================

#[tokio::test]
async fn test_stripe_missing_api_key() {
    let connector = StripeConnector::new();

    let config = serde_json::json!({});
    let input = serde_json::json!({});
    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "createCustomer", &input, &opts)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_slack_missing_token() {
    let connector = SlackConnector::new();

    let config = serde_json::json!({});
    let input = serde_json::json!({
        "channel": "C12345678",
        "text": "Test"
    });
    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "postMessage", &input, &opts)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_github_missing_token() {
    let connector = GitHubConnector::new();

    let config = serde_json::json!({
        "owner": "test",
        "repo": "test"
    });
    let input = serde_json::json!({
        "title": "Test"
    });
    let opts = ActionOpts {
        dry_run: true,
        idempotency_key: None,
    };

    let result = connector
        .invoke_action(&config, "createIssue", &input, &opts)
        .await;

    assert!(result.is_err());
}
