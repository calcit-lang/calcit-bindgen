{} (:command |ffi.export)
  :interface-schema |https://calcit-lang.org/schemas/component-interface-ir-v1.schema.json
  :revision |md5:549ef3ed95f3cda9190b6512f270f7a1
  :schema-version 1
  :data $ {}
    :filters $ {} (:boundary |component) (:include-dependencies false) (:namespace nil)
    :interface $ {} (:package |component-wasm) (:package-version |0.0.0) (:version 1)
      :declarations $ []
      :definitions $ []
        {} (:direction |export) (:doc |) (:id |component-wasm.main/add-one)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Number)) (:return 'Number)))"
          :module nil
          :name |add-one
          :namespace |component-wasm.main
          :status |supported
          :symbol |add-one
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |number
            :result $ {} $ :kind |number
        {} (:direction |export) (:doc |) (:id |component-wasm.main/call-host-add-one)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Number)) (:return 'Number)))"
          :module nil
          :name |call-host-add-one
          :namespace |component-wasm.main
          :status |supported
          :symbol |call-host-add-one
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |number
            :result $ {} $ :kind |number
        {} (:direction |export) (:doc |) (:id |component-wasm.main/call-host-echo)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'String)) (:return 'String)))"
          :module nil
          :name |call-host-echo
          :namespace |component-wasm.main
          :status |supported
          :symbol |call-host-echo
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |string
            :result $ {} $ :kind |string
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-text)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'String)) (:return 'String)))"
          :module nil
          :name |echo-text
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-text
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |string
            :result $ {} $ :kind |string
        {} (:direction |import) (:doc |) (:id |component-wasm.main/host-add-one)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Number)) (:return 'Number)))"
          :module |host
          :name |host-add-one
          :namespace |component-wasm.main
          :status |supported
          :symbol |add-one
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |number
            :result $ {} $ :kind |number
        {} (:direction |import) (:doc |) (:id |component-wasm.main/host-echo)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'String)) (:return 'String)))"
          :module |host
          :name |host-echo
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |string
            :result $ {} $ :kind |string
    :summary $ {} (:definitions 6) (:diagnostics 0) (:supported 6) (:unsupported 0)
  :diagnostics $ []
