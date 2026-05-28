/**
 * Reactor Bun Function Shim
 *
 * This shim wraps user code and exposes it via a Unix socket for the Reactor
 * runtime to invoke. It expects user code at ./code/index.ts to export:
 *
 *   export default { fetch(req: Request): Response | Promise<Response> }
 *
 * Environment variables:
 * - REACTOR_SOCKET_PATH: Unix socket path for the HTTP server
 * - REACTOR_FUNCTION_NAME: Name of the function
 * - REACTOR_DEPLOYMENT_ID: Deployment ID
 * - REACTOR_TIMEOUT_MS: Request timeout in milliseconds
 * - REACTOR_MAX_BODY_IN_BYTES: Max request body size
 * - REACTOR_MAX_BODY_OUT_BYTES: Max response body size
 */

const SOCKET_PATH = process.env.REACTOR_SOCKET_PATH;
const FUNCTION_NAME = process.env.REACTOR_FUNCTION_NAME ?? "unknown";
const DEPLOYMENT_ID = process.env.REACTOR_DEPLOYMENT_ID ?? "unknown";
const TIMEOUT_MS = parseInt(process.env.REACTOR_TIMEOUT_MS ?? "30000", 10);
const MAX_BODY_IN = parseInt(process.env.REACTOR_MAX_BODY_IN_BYTES ?? "6291456", 10);

if (!SOCKET_PATH) {
  console.error("REACTOR_SOCKET_PATH not set");
  process.exit(1);
}

// Import user code
let userModule: { default: { fetch: (req: Request) => Response | Promise<Response> } };

try {
  // The bundle structure is:
  //   /path/to/deployment/
  //     code/
  //       index.ts (user entrypoint)
  //     manifest.json
  //     shim.ts (this file, copied here)
  userModule = await import("./code/index.ts");
} catch (err) {
  console.error(`Failed to import user code: ${err}`);
  process.exit(1);
}

const userFetch = userModule.default?.fetch;

if (typeof userFetch !== "function") {
  console.error("User code must export default { fetch(req): Response }");
  process.exit(1);
}

// Create an abort controller for graceful shutdown
const shutdownController = new AbortController();

// Handle graceful shutdown
process.on("SIGTERM", () => {
  console.log(`[${FUNCTION_NAME}] Received SIGTERM, shutting down gracefully`);
  shutdownController.abort();
});

process.on("SIGINT", () => {
  console.log(`[${FUNCTION_NAME}] Received SIGINT, shutting down gracefully`);
  shutdownController.abort();
});

// Remove stale socket if exists (Bun doesn't auto-remove)
try {
  await Bun.file(SOCKET_PATH).exists() && (await Bun.write(SOCKET_PATH, ""));
} catch {
  // Ignore - socket may not exist
}

// Start the Unix socket HTTP server
const server = Bun.serve({
  unix: SOCKET_PATH,
  
  // Main fetch handler
  async fetch(req: Request): Promise<Response> {
    const requestId = req.headers.get("x-request-id") ?? crypto.randomUUID();
    const startTime = Date.now();
    
    // Create per-request abort controller with timeout
    const requestController = new AbortController();
    const timeoutId = setTimeout(() => {
      requestController.abort(new Error("Request timeout"));
    }, TIMEOUT_MS);
    
    // Also abort if shutdown is signaled
    shutdownController.signal.addEventListener("abort", () => {
      requestController.abort(new Error("Server shutdown"));
    });
    
    try {
      // Check request body size
      const contentLength = req.headers.get("content-length");
      if (contentLength && parseInt(contentLength, 10) > MAX_BODY_IN) {
        return new Response(
          JSON.stringify({ error: "Request body too large" }),
          {
            status: 413,
            headers: { "content-type": "application/json" },
          }
        );
      }
      
      // Create a new request with our signal attached
      // This allows user code to respect timeouts and cancellation
      const signalledRequest = new Request(req.url, {
        method: req.method,
        headers: req.headers,
        body: req.body,
        signal: requestController.signal,
      });
      
      // Call user's fetch handler
      const response = await userFetch(signalledRequest);
      
      // Add Reactor headers to response
      const headers = new Headers(response.headers);
      headers.set("x-reactor-function", FUNCTION_NAME);
      headers.set("x-reactor-deployment", DEPLOYMENT_ID);
      headers.set("x-reactor-duration-ms", String(Date.now() - startTime));
      headers.set("x-request-id", requestId);
      
      // Return response with added headers
      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers,
      });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error(`[${FUNCTION_NAME}] Request error: ${errorMessage}`);
      
      // Check if it's an abort error
      if (err instanceof DOMException && err.name === "AbortError") {
        return new Response(
          JSON.stringify({ error: "Request aborted", message: errorMessage }),
          {
            status: 504,
            headers: { "content-type": "application/json" },
          }
        );
      }
      
      // Return error response
      return new Response(
        JSON.stringify({ error: "Internal function error", message: errorMessage }),
        {
          status: 500,
          headers: { "content-type": "application/json" },
        }
      );
    } finally {
      clearTimeout(timeoutId);
    }
  },
  
  // Error handler
  error(error: Error): Response {
    console.error(`[${FUNCTION_NAME}] Server error: ${error.message}`);
    return new Response(
      JSON.stringify({ error: "Server error", message: error.message }),
      {
        status: 500,
        headers: { "content-type": "application/json" },
      }
    );
  },
});

console.log(`[${FUNCTION_NAME}] Listening on ${SOCKET_PATH}`);

// Keep the process alive until shutdown signal
await new Promise<void>((resolve) => {
  shutdownController.signal.addEventListener("abort", () => {
    server.stop();
    resolve();
  });
});

console.log(`[${FUNCTION_NAME}] Server stopped`);
