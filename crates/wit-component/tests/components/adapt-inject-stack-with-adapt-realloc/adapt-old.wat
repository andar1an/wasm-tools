(module
  (import "new" "get-two" (func $get_two (param i32)))
  (import "__main_module__" "cabi_realloc" (func $cabi_realloc (param i32 i32 i32 i32) (result i32)))
  (import "env" "memory" (memory 0))

  (global $__stack_pointer (mut i32) i32.const 0)
  (global $some_other_mutable_global (mut i32) i32.const 0)

  ;; `wit-component` should use this to track the status of a lazy stack
  ;; allocation:
  (global $allocation_state (mut i32) i32.const 0)

  ;; This is a sample adapter which is adapting between ABI. This exact function
  ;; signature is imported by `module.wat` and we're implementing it here with a
  ;; canonical-abi function that returns two integers. The canonical ABI for
  ;; returning two integers is different than the ABI of this function, hence
  ;; the adapter here.
  ;;
  ;; The purpose of this test case is to exercise the `$__stack_pointer` global.
  ;; The stack pointer here needs to be initialized to something valid for
  ;; this adapter module which is done with an injected `start` function into
  ;; this adapter module when it's bundled into a component.
  (func (export "get_sum") (result i32)
    (local i32 i32)

    ;; `wit-component` should have injected a call to a function that allocates
    ;; the stack and sets $allocation_state to 2
    (if (i32.ne (global.get $allocation_state) (i32.const 2)) (then (unreachable)))

    ;; First, allocate a page using $cabi_realloc and write to it.  This tests
    ;; that we can use the main module's allocator if present (or else a
    ;; substitute synthesized by `wit-component`).
    (local.set 0
      (call $cabi_realloc
        (i32.const 0)
        (i32.const 0)
        (i32.const 8)
        (i32.const 65536)))

    (i32.store (local.get 0) (i32.const 42))
    (i32.store offset=65532 (local.get 0) (i32.const 42))

    ;; Allocate 8 bytes of stack space for the two u32 return values. The
    ;; original stack pointer is saved in local 0 and the stack frame for this
    ;; function is saved in local 1.
    global.get $__stack_pointer
    local.tee 0
    i32.const 8
    i32.sub
    local.tee 1
    global.set $__stack_pointer

    ;; Call the imported function which will return two u32 values into the
    ;; return pointer specified here, our stack frame.
    local.get 1
    call $get_two

    ;; Compute the result of this function by adding together the two return
    ;; values.
    (i32.add
      (i32.load (local.get 1))
      (i32.load offset=4 (local.get 1)))

    ;; Test that if there is another mutable global in this module that it
    ;; doesn't affect the detection of the stack pointer. This extra mutable
    ;; global should not be initialized or tampered with as part of the
    ;; initialize-the-stack-pointer injected function
    (global.set $some_other_mutable_global (global.get $some_other_mutable_global))

    ;; Restore the stack pointer to the value it was at prior to entering this
    ;; function.
    local.get 0
    global.set $__stack_pointer
  )

)
