{} (:command |ffi.export)
  :interface-schema |https://calcit-lang.org/schemas/component-interface-ir-v1.schema.json
  :revision |md5:1faa5380579098a2d0660ca73f4dd529
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/call-host-buffer)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Buffer)) (:return 'Buffer)))"
          :module nil
          :name |call-host-buffer
          :namespace |component-wasm.main
          :status |supported
          :symbol |call-host-buffer
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |buffer
            :result $ {} $ :kind |buffer
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/call-host-numbers)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'Number))) (:return (:: 'List 'Number))))"
          :module nil
          :name |call-host-numbers
          :namespace |component-wasm.main
          :status |supported
          :symbol |call-host-numbers
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |number
            :result $ {} (:kind |list)
              :item $ {} $ :kind |number
        {} (:direction |export) (:doc |) (:id |component-wasm.main/choose-buffer)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Bool 'Buffer 'Buffer)) (:return 'Buffer)))"
          :module nil
          :name |choose-buffer
          :namespace |component-wasm.main
          :status |supported
          :symbol |choose-buffer
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ []
              {} (:position 0)
                :type $ {} $ :kind |bool
              {} (:position 1)
                :type $ {} $ :kind |buffer
              {} (:position 2)
                :type $ {} $ :kind |buffer
            :result $ {} $ :kind |buffer
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-bools)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'Bool))) (:return (:: 'List 'Bool))))"
          :module nil
          :name |echo-bools
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-bools
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |bool
            :result $ {} (:kind |list)
              :item $ {} $ :kind |bool
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-buffer)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Buffer)) (:return 'Buffer)))"
          :module nil
          :name |echo-buffer
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-buffer
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |buffer
            :result $ {} $ :kind |buffer
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-buffers)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'Buffer))) (:return (:: 'List 'Buffer))))"
          :module nil
          :name |echo-buffers
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-buffers
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |buffer
            :result $ {} (:kind |list)
              :item $ {} $ :kind |buffer
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-number-lists)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List (:: 'List 'Number)))) (:return (:: 'List (:: 'List 'Number)))))"
          :module nil
          :name |echo-number-lists
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-number-lists
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} (:kind |list)
                  :item $ {} $ :kind |number
            :result $ {} (:kind |list)
              :item $ {} (:kind |list)
                :item $ {} $ :kind |number
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-numbers)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'Number))) (:return (:: 'List 'Number))))"
          :module nil
          :name |echo-numbers
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-numbers
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |number
            :result $ {} (:kind |list)
              :item $ {} $ :kind |number
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
        {} (:direction |export) (:doc |) (:id |component-wasm.main/echo-texts)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'String))) (:return (:: 'List 'String))))"
          :module nil
          :name |echo-texts
          :namespace |component-wasm.main
          :status |supported
          :symbol |echo-texts
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |string
            :result $ {} (:kind |list)
              :item $ {} $ :kind |string
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
        {} (:direction |import) (:doc |) (:id |component-wasm.main/host-buffer)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Buffer)) (:return 'Buffer)))"
          :module |host
          :name |host-buffer
          :namespace |component-wasm.main
          :status |supported
          :symbol |buffer
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |buffer
            :result $ {} $ :kind |buffer
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
        {} (:direction |import) (:doc |) (:id |component-wasm.main/host-numbers)
          :logical-schema "|(:: 'Fn ({} (:args ([] (:: 'List 'Number))) (:return (:: 'List 'Number))))"
          :module |host
          :name |host-numbers
          :namespace |component-wasm.main
          :status |supported
          :symbol |numbers
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} (:kind |list)
                :item $ {} $ :kind |number
            :result $ {} (:kind |list)
              :item $ {} $ :kind |number
        {} (:direction |export) (:doc |) (:id |component-wasm.main/is-buffer)
          :logical-schema "|(:: 'Fn ({} (:args ([] 'Buffer)) (:return 'Bool)))"
          :module nil
          :name |is-buffer
          :namespace |component-wasm.main
          :status |supported
          :symbol |is-buffer
          :diagnostic-codes $ []
          :signature $ {}
            :parameters $ [] $ {} (:position 0)
              :type $ {} $ :kind |buffer
            :result $ {} $ :kind |bool
    :summary $ {} (:definitions 22) (:diagnostics 0) (:supported 22) (:unsupported 0)
  :diagnostics $ []
