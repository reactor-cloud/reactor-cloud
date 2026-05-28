;; Minimal WASM module for smoke test
;; Exports a simple memory and a function that returns "hello"

(module
  ;; Export memory for the host to read
  (memory (export "memory") 1)

  ;; Store "hello, reactor" at memory offset 0
  (data (i32.const 0) "hello, reactor")

  ;; Return the length of the greeting (14 bytes)
  (func (export "greet") (result i32)
    i32.const 14
  )

  ;; Return the memory offset where greeting starts
  (func (export "greet_ptr") (result i32)
    i32.const 0
  )
)
