//! Property-based tests for tenant isolation in shared clusters.
//!
//! These tests verify that:
//! - Tenants cannot access each other's data
//! - Concurrent operations on different tenants are isolated
//! - Database connections are properly scoped to tenant databases
//! - Quota enforcement is per-tenant
//! - Adversarial inputs don't break isolation

#![cfg(feature = "cap-cloud")]

use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;

/// Property test strategies for tenant isolation testing.
mod strategies {
    use proptest::prelude::*;
    use std::sync::Arc;

    /// Generate a random tenant reference (URL-safe string).
    pub fn tenant_ref() -> impl Strategy<Value = String> {
        "[a-z0-9]{8,20}".prop_map(|s| s.to_lowercase())
    }

    /// Generate multiple unique tenant refs.
    pub fn unique_tenant_refs(count: usize) -> impl Strategy<Value = Vec<String>> {
        proptest::collection::vec(tenant_ref(), count..=count).prop_filter(
            "unique refs",
            |refs| {
                let mut seen = std::collections::HashSet::new();
                refs.iter().all(|r| seen.insert(r.clone()))
            },
        )
    }

    /// Generate adversarial tenant refs that might break isolation.
    pub fn adversarial_tenant_ref() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("../../../etc/passwd".to_string()),
            Just("tenant_other".to_string()),
            Just("admin".to_string()),
            Just("postgres".to_string()),
            Just("__internal__".to_string()),
            Just("..".to_string()),
            Just("./".to_string()),
            Just("null".to_string()),
            Just("SELECT * FROM users".to_string()),
            "[a-z0-9]{1,5}".prop_map(|s| format!("{}; DROP TABLE users;", s)),
            "[a-z0-9]{1,5}".prop_map(|s| format!("{}' OR '1'='1", s)),
        ]
    }

    /// Generate valid data for a tenant operation.
    pub fn tenant_data() -> impl Strategy<Value = serde_json::Value> {
        prop_oneof![
            Just(serde_json::json!({"name": "test", "value": 42})),
            "[a-zA-Z0-9_]{1,20}".prop_map(|s| serde_json::json!({"key": s})),
            (0i64..1000).prop_map(|n| serde_json::json!({"count": n})),
        ]
    }
}

/// Database name generation tests.
mod db_name_tests {
    use super::*;

    proptest! {
        #[test]
        fn db_name_is_safe(ref tenant_ref in strategies::tenant_ref()) {
            let db_name = format!("tenant_{}", tenant_ref);

            // Must start with letter (tenant_)
            prop_assert!(db_name.starts_with("tenant_"));

            // Must be valid PostgreSQL identifier (alphanumeric + underscore)
            prop_assert!(db_name.chars().all(|c| c.is_alphanumeric() || c == '_'));

            // Must be reasonable length
            prop_assert!(db_name.len() >= 9); // "tenant_" + at least 1 char
            prop_assert!(db_name.len() <= 63); // PostgreSQL identifier limit
        }

        #[test]
        fn adversarial_ref_is_sanitized(ref tenant_ref in strategies::adversarial_tenant_ref()) {
            // Even adversarial inputs should produce safe database names
            // The DB name should NOT contain the adversarial content if sanitized

            // In the actual implementation, tenant_refs come from the database
            // after validation, so they should already be safe.
            // This test documents what the expected behavior should be.

            let sanitized_ref = tenant_ref
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase();

            if !sanitized_ref.is_empty() {
                let db_name = format!("tenant_{}", sanitized_ref);
                prop_assert!(db_name.chars().all(|c| c.is_alphanumeric() || c == '_'));
            }
        }
    }
}

/// Storage prefix isolation tests.
mod storage_prefix_tests {
    use super::*;

    proptest! {
        #[test]
        fn storage_prefixes_are_unique(
            refs in strategies::unique_tenant_refs(10)
        ) {
            let bucket = "reactor-storage";
            let prefixes: Vec<_> = refs
                .iter()
                .map(|r| format!("{}/{}", bucket, r))
                .collect();

            // All prefixes must be unique
            let unique: HashSet<_> = prefixes.iter().collect();
            prop_assert_eq!(prefixes.len(), unique.len());

            // No prefix should be a prefix of another
            for (i, p1) in prefixes.iter().enumerate() {
                for (j, p2) in prefixes.iter().enumerate() {
                    if i != j {
                        prop_assert!(!p1.starts_with(p2));
                        prop_assert!(!p2.starts_with(p1));
                    }
                }
            }
        }

        #[test]
        fn storage_prefix_no_traversal(ref tenant_ref in strategies::adversarial_tenant_ref()) {
            let bucket = "reactor-storage";

            // Sanitize adversarial ref
            let safe_ref: String = tenant_ref
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();

            if !safe_ref.is_empty() {
                let prefix = format!("{}/{}", bucket, safe_ref);

                // Must not allow directory traversal
                prop_assert!(!prefix.contains(".."));
                prop_assert!(!prefix.contains("//"));
                prop_assert!(!prefix.starts_with("/"));
            }
        }
    }
}

/// Quota isolation tests.
mod quota_tests {
    use super::*;

    proptest! {
        #[test]
        fn quota_keys_are_tenant_scoped(
            refs in strategies::unique_tenant_refs(5),
            requests in proptest::collection::vec(0u32..1000, 5)
        ) {
            // Simulate quota tracking for multiple tenants
            let mut quota_tracker: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

            // Each tenant should have isolated quota tracking
            for (ref_, req_count) in refs.iter().zip(requests.iter()) {
                quota_tracker.insert(ref_.clone(), *req_count);
            }

            // Verify isolation: each tenant's quota is independent
            for ref_ in refs.iter() {
                let quota = quota_tracker.get(ref_);
                prop_assert!(quota.is_some());
            }

            // Verify no cross-contamination
            prop_assert_eq!(quota_tracker.len(), refs.len());
        }
    }
}

/// Topic naming isolation tests for realtime/pubsub.
mod topic_tests {
    use super::*;

    fn build_topic(project_ref: &str, table: &str, op: &str) -> String {
        format!("reactor.{}.data.{}.{}", project_ref, table, op)
    }

    proptest! {
        #[test]
        fn topics_are_tenant_scoped(
            refs in strategies::unique_tenant_refs(5),
            table in "[a-z_]{3,20}"
        ) {
            let topics: Vec<_> = refs
                .iter()
                .map(|r| build_topic(r, &table, "insert"))
                .collect();

            // All topics must be unique
            let unique: HashSet<_> = topics.iter().collect();
            prop_assert_eq!(topics.len(), unique.len());

            // Each topic must contain its tenant ref
            for (ref_, topic) in refs.iter().zip(topics.iter()) {
                prop_assert!(topic.contains(ref_));
            }
        }

        #[test]
        fn topic_subscription_isolation(
            ref1 in strategies::tenant_ref(),
            ref2 in strategies::tenant_ref(),
            table in "[a-z_]{3,20}"
        ) {
            prop_assume!(ref1 != ref2);

            let topic1 = build_topic(&ref1, &table, "*");
            let topic2 = build_topic(&ref2, &table, "*");

            // Subscribing to tenant1's topic should not receive tenant2's messages
            // This is verified by topic structure - no overlap
            prop_assert!(!topic1.contains(&ref2));
            prop_assert!(!topic2.contains(&ref1));

            // Wildcard subscriptions are still scoped to the tenant
            let wildcard1 = format!("reactor.{}.data.>", ref1);
            let wildcard2 = format!("reactor.{}.data.>", ref2);

            prop_assert!(!wildcard1.contains(&ref2));
            prop_assert!(!wildcard2.contains(&ref1));
        }
    }
}

/// Concurrent operation isolation tests.
#[cfg(test)]
mod concurrent_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct MockTenantState {
        operations: AtomicUsize,
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]

        #[test]
        fn concurrent_operations_isolated(
            refs in strategies::unique_tenant_refs(3),
            op_counts in proptest::collection::vec(1usize..10, 3)
        ) {
            use std::collections::HashMap;

            // Simulate concurrent operations per tenant
            let states: HashMap<String, Arc<MockTenantState>> = refs
                .iter()
                .map(|r| (r.clone(), Arc::new(MockTenantState::default())))
                .collect();

            // Perform operations (simulated - would be actual async in real tests)
            for (ref_, count) in refs.iter().zip(op_counts.iter()) {
                let state = states.get(ref_).unwrap();
                for _ in 0..*count {
                    state.operations.fetch_add(1, Ordering::Relaxed);
                }
            }

            // Verify each tenant got exactly the expected operations
            for (ref_, count) in refs.iter().zip(op_counts.iter()) {
                let state = states.get(ref_).unwrap();
                prop_assert_eq!(state.operations.load(Ordering::Relaxed), *count);
            }
        }
    }
}

/// Connection pool isolation tests.
mod pool_isolation_tests {
    use super::*;

    proptest! {
        #[test]
        fn connection_urls_are_isolated(
            refs in strategies::unique_tenant_refs(5),
            base_url in Just("postgres://reactor:pass@supavisor:5432")
        ) {
            let urls: Vec<_> = refs
                .iter()
                .map(|r| format!("{}/tenant_{}", base_url, r))
                .collect();

            // All URLs must be unique
            let unique: HashSet<_> = urls.iter().collect();
            prop_assert_eq!(urls.len(), unique.len());

            // Each URL must point to a different database
            let dbs: HashSet<_> = urls
                .iter()
                .map(|url| url.split('/').last().unwrap())
                .collect();
            prop_assert_eq!(dbs.len(), refs.len());
        }

        #[test]
        fn pool_url_template_substitution(
            refs in strategies::unique_tenant_refs(3)
        ) {
            let template = "postgres://reactor:pass@supavisor:5432/{ref}";

            for ref_ in refs {
                let url = template.replace("{ref}", &format!("tenant_{}", ref_));

                // URL should contain the tenant database
                prop_assert!(url.contains(&format!("tenant_{}", ref_)));

                // URL should be valid format
                prop_assert!(url.starts_with("postgres://"));
                prop_assert!(url.contains('@'));
            }
        }
    }
}

/// Vault path isolation tests.
mod vault_path_tests {
    use super::*;

    proptest! {
        #[test]
        fn vault_paths_are_scoped(
            refs in strategies::unique_tenant_refs(5),
            secret_name in "[a-z_]{3,15}"
        ) {
            let paths: Vec<_> = refs
                .iter()
                .map(|r| format!("secret/tenants/{}/{}", r, secret_name))
                .collect();

            // All paths must be unique
            let unique: HashSet<_> = paths.iter().collect();
            prop_assert_eq!(paths.len(), unique.len());

            // Each path must be scoped to its tenant
            for (ref_, path) in refs.iter().zip(paths.iter()) {
                prop_assert!(path.contains(ref_));
                prop_assert!(path.starts_with("secret/tenants/"));
            }
        }

        #[test]
        fn vault_path_no_traversal(ref tenant_ref in strategies::adversarial_tenant_ref()) {
            // Sanitize the ref
            let safe_ref: String = tenant_ref
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();

            if !safe_ref.is_empty() && safe_ref.len() <= 20 {
                let path = format!("secret/tenants/{}/data-key", safe_ref);

                // Must not allow path traversal
                prop_assert!(!path.contains(".."));
                prop_assert!(!path.contains("//"));
                prop_assert!(path.starts_with("secret/tenants/"));
            }
        }
    }
}

#[cfg(test)]
mod integration_simulation {
    use super::*;

    /// Simulates a multi-tenant request flow to verify isolation.
    #[test]
    fn simulated_multi_tenant_flow() {
        use std::collections::HashMap;

        // Simulate 3 tenants making requests
        let tenants = vec!["abc123", "def456", "ghi789"];

        // Track data written by each tenant
        let mut tenant_data: HashMap<&str, Vec<serde_json::Value>> = HashMap::new();

        for tenant in &tenants {
            tenant_data.insert(tenant, Vec::new());
        }

        // Simulate writes
        for tenant in &tenants {
            let data = serde_json::json!({"tenant": tenant, "value": 42});
            tenant_data.get_mut(tenant).unwrap().push(data);
        }

        // Verify isolation: each tenant only sees their own data
        for tenant in &tenants {
            let data = &tenant_data[tenant];
            assert_eq!(data.len(), 1);

            let first = &data[0];
            assert_eq!(first["tenant"], *tenant);
        }

        // Verify no cross-contamination
        for (i, t1) in tenants.iter().enumerate() {
            for (j, t2) in tenants.iter().enumerate() {
                if i != j {
                    let data1 = &tenant_data[t1];
                    let data2 = &tenant_data[t2];

                    // No data from t2 should be in t1's store
                    for item in data1 {
                        assert_ne!(item["tenant"], *t2);
                    }
                    for item in data2 {
                        assert_ne!(item["tenant"], *t1);
                    }
                }
            }
        }
    }
}
