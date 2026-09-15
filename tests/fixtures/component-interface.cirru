{} (:command |ffi.export)
  :interface-schema |https://calcit-lang.org/schemas/component-interface-ir-v1.schema.json
  :revision |md5:723ae5f9dce923788f20524cf72db554
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/bool-not)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Bool)) (:return 'Bool)))"
          :module nil
          :name |bool-not
          :namespace |component-wasm.main
          :status |supported
          :symbol |bool-not
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |bool
            :result $ {} $ :kind |bool
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/call-host-bool-not)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Bool)) (:return 'Bool)))"
          :module nil
          :name |call-host-bool-not
          :namespace |component-wasm.main
          :status |supported
          :symbol |call-host-bool-not
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |bool
            :result $ {} $ :kind |bool
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/choose-number)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Bool 'Number 'Number)) (:return 'Number)))"
          :module nil
          :name |choose-number
          :namespace |component-wasm.main
          :status |supported
          :symbol |choose-number
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ []
              {} (:position 0)
                :type $ {} $ :kind |bool
              {} (:position 1)
                :type $ {} $ :kind |number
              {} (:position 2)
                :type $ {} $ :kind |number
            :result $ {} $ :kind |number
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
        {} (:direction |import) (:doc |) (:id |component-wasm.main/host-bool-not)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Bool)) (:return 'Bool)))"
          :module |host
          :name |host-bool-not
          :namespace |component-wasm.main
          :status |supported
          :symbol |bool-not
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |bool
            :result $ {} $ :kind |bool
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
    :summary $ {} (:definitions 10) (:diagnostics 0) (:supported 10) (:unsupported 0)
  :diagnostics $ []
