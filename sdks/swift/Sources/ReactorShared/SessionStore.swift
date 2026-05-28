import Foundation
#if canImport(Security)
import Security
#endif

/// Protocol for session persistence.
public protocol SessionStore: Sendable {
    func getItem(_ key: String) async -> String?
    func setItem(_ key: String, value: String) async
    func removeItem(_ key: String) async
}

/// In-memory session store.
/// Useful for testing or when no persistent storage is available.
public actor InMemorySessionStore: SessionStore {
    private var store: [String: String] = [:]
    
    public init() {}
    
    public func getItem(_ key: String) -> String? {
        store[key]
    }
    
    public func setItem(_ key: String, value: String) {
        store[key] = value
    }
    
    public func removeItem(_ key: String) {
        store[key] = nil
    }
}

#if canImport(Security)
/// Keychain-based session store for Apple platforms.
/// Uses kSecAttrAccessibleAfterFirstUnlock for security.
public final class KeychainSessionStore: SessionStore, @unchecked Sendable {
    private let service: String
    private let accessGroup: String?
    private let queue = DispatchQueue(label: "cloud.reactor.keychain", qos: .userInitiated)
    
    /// Create a Keychain session store.
    ///
    /// - Parameters:
    ///   - service: Keychain service identifier (default: "cloud.reactor.session")
    ///   - accessGroup: Optional keychain access group for app extensions
    public init(service: String = "cloud.reactor.session", accessGroup: String? = nil) {
        self.service = service
        self.accessGroup = accessGroup
    }
    
    public func getItem(_ key: String) async -> String? {
        await withCheckedContinuation { continuation in
            queue.async { [self] in
                var query: [CFString: Any] = [
                    kSecClass: kSecClassGenericPassword,
                    kSecAttrService: service,
                    kSecAttrAccount: key,
                    kSecReturnData: true,
                    kSecMatchLimit: kSecMatchLimitOne
                ]
                
                if let accessGroup {
                    query[kSecAttrAccessGroup] = accessGroup
                }
                
                var result: AnyObject?
                let status = SecItemCopyMatching(query as CFDictionary, &result)
                
                guard status == errSecSuccess,
                      let data = result as? Data,
                      let string = String(data: data, encoding: .utf8) else {
                    continuation.resume(returning: nil)
                    return
                }
                
                continuation.resume(returning: string)
            }
        }
    }
    
    public func setItem(_ key: String, value: String) async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            queue.async { [self] in
                guard let data = value.data(using: .utf8) else {
                    continuation.resume()
                    return
                }
                
                var query: [CFString: Any] = [
                    kSecClass: kSecClassGenericPassword,
                    kSecAttrService: service,
                    kSecAttrAccount: key
                ]
                
                if let accessGroup {
                    query[kSecAttrAccessGroup] = accessGroup
                }
                
                let attributes: [CFString: Any] = [
                    kSecValueData: data,
                    kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlock
                ]
                
                var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
                
                if status == errSecItemNotFound {
                    var newItem = query
                    newItem.merge(attributes) { $1 }
                    status = SecItemAdd(newItem as CFDictionary, nil)
                }
                
                continuation.resume()
            }
        }
    }
    
    public func removeItem(_ key: String) async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            queue.async { [self] in
                var query: [CFString: Any] = [
                    kSecClass: kSecClassGenericPassword,
                    kSecAttrService: service,
                    kSecAttrAccount: key
                ]
                
                if let accessGroup {
                    query[kSecAttrAccessGroup] = accessGroup
                }
                
                _ = SecItemDelete(query as CFDictionary)
                continuation.resume()
            }
        }
    }
}
#endif

/// Default session store key for sessions.
public let defaultSessionKey = "reactor.session"
