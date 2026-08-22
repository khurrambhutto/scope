foreach(test_name test_safety test_parsers test_planstore)
    add_executable(${test_name} ${test_name}.cpp)
    target_link_libraries(${test_name} PRIVATE scope_core Qt6::Test)
    add_test(NAME ${test_name} COMMAND ${test_name})
endforeach()

# Manual smoke harness: runs a real scan + plan previews against this machine.
add_executable(smoke_scan smoke_scan.cpp)
target_link_libraries(smoke_scan PRIVATE scope_core Qt6::Core)
