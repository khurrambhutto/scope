cmake_minimum_required(VERSION 3.24)

project(ScopeQt VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

add_compile_options(-Wall -Wextra -Wno-unused-parameter)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Concurrent Test)

# ---------------------------------------------------------------------------
# scope_core: UI-free library (scanners, safety, operations, system helpers)
# ---------------------------------------------------------------------------
add_library(scope_core STATIC
    src/core/package/Package.cpp
    src/core/system/System.cpp
    src/core/safety/Safety.cpp
    src/core/desktop/DesktopEntry.cpp
    src/core/icons/IconResolver.cpp
    src/core/scanner/AptScanner.cpp
    src/core/scanner/SnapScanner.cpp
    src/core/scanner/FlatpakScanner.cpp
    src/core/scanner/AppImageScanner.cpp
    src/core/scanner/ScanService.cpp
    src/core/operations/OperationTypes.cpp
    src/core/operations/PlanStore.cpp
    src/core/operations/UninstallOp.cpp
    src/core/operations/UpdateOp.cpp
)

target_include_directories(scope_core PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/src/core
)
target_link_libraries(scope_core PUBLIC Qt6::Core Qt6::Gui Qt6::Concurrent)

# ---------------------------------------------------------------------------
# scope: desktop app
# ---------------------------------------------------------------------------
qt_standard_project_setup()

qt_add_executable(scope
    src/main.cpp
    src/app/PackagesController.cpp
    src/app/OperationController.cpp
    src/app/TaskModel.cpp
    src/models/PackageListModel.cpp
    src/icons/IconProvider.cpp
)

qt_add_qml_module(scope
    URI scope
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/PackagesPage.qml
        qml/TaskCenterPage.qml
        qml/PackageRow.qml
        qml/DetailDrawer.qml
        qml/PreviewDialog.qml
)
target_sources(scope PRIVATE src/qml/Theme.h)

target_link_libraries(scope PRIVATE
    scope_core
    Qt6::Qml
    Qt6::Quick
    Qt6::QuickControls2
)

set_target_properties(scope PROPERTIES RUNTIME_OUTPUT_NAME scope-app)

set_source_files_properties(qml/Theme.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

# ---------------------------------------------------------------------------
# tests
# ---------------------------------------------------------------------------
enable_testing()
add_subdirectory(tests)
