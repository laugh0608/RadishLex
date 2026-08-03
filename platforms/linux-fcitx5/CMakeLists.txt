cmake_minimum_required(VERSION 3.21)
project(radishlex-fcitx5 VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

get_filename_component(RADISHLEX_REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.."
                       ABSOLUTE)
set(RADISHLEX_IME_FFI_INCLUDE_DIR
    "${RADISHLEX_REPO_ROOT}/crates/ime-ffi/include"
    CACHE PATH "Directory containing radishlex_input.h")
set(RADISHLEX_RIME_DATA_SOURCE_DIR
    "${RADISHLEX_REPO_ROOT}/packaging/rime/data"
    CACHE PATH "Prepared immutable RadishLex RimeData source")
set(RADISHLEX_RIME_SCHEMA "radishlex_pinyin"
    CACHE STRING "Locked RadishLex Rime schema")
set(RADISHLEX_RIME_ASSETS
  default.yaml
  radishlex_pinyin.schema.yaml
  pinyin_simp.dict.yaml
)

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  set(RADISHLEX_FCITX_ADDON_DEFAULT ON)
else()
  set(RADISHLEX_FCITX_ADDON_DEFAULT OFF)
endif()
option(RADISHLEX_BUILD_FCITX_ADDON "Build the Linux Fcitx5 shared addon"
       ${RADISHLEX_FCITX_ADDON_DEFAULT})
option(RADISHLEX_BUILD_CONTRACT_TESTS "Build platform-independent contracts" ON)

add_library(radishlex_linux_platform STATIC
  src/application_context.cpp
  src/ffi_projection.cpp
  src/key_projection.cpp
  src/linked_ffi_api.cpp
  src/manager_runtime.cpp
  src/privacy_mode.cpp
  src/runtime_layout.cpp
  src/xdg_paths.cpp
)
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  target_sources(radishlex_linux_platform PRIVATE
    src/privacy_monitor.cpp
  )
endif()
target_include_directories(radishlex_linux_platform
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${RADISHLEX_IME_FFI_INCLUDE_DIR}"
)
target_compile_options(radishlex_linux_platform PRIVATE
  -Wall -Wextra -Wpedantic -Werror
)
if(CMAKE_DL_LIBS)
  target_link_libraries(radishlex_linux_platform PUBLIC "${CMAKE_DL_LIBS}")
endif()
set_target_properties(radishlex_linux_platform PROPERTIES
  POSITION_INDEPENDENT_CODE ON
)

if(RADISHLEX_BUILD_FCITX_ADDON)
  if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "The Fcitx5 addon target is Linux-only")
  endif()
  find_package(Fcitx5Core 5.1.9 REQUIRED)
  include("${FCITX_INSTALL_CMAKECONFIG_DIR}/Fcitx5Utils/Fcitx5CompilerSettings.cmake")

  set(RADISHLEX_IME_FFI_LIBRARY "" CACHE FILEPATH
      "native-rime lib radishlex_ime_ffi shared library")
  if(NOT EXISTS "${RADISHLEX_IME_FFI_LIBRARY}")
    message(FATAL_ERROR
      "RADISHLEX_IME_FFI_LIBRARY must identify the native-rime cdylib")
  endif()
  foreach(asset IN LISTS RADISHLEX_RIME_ASSETS)
    if(NOT EXISTS "${RADISHLEX_RIME_DATA_SOURCE_DIR}/${asset}")
      message(FATAL_ERROR "missing locked RimeData asset: ${asset}")
    endif()
  endforeach()

  add_library(radishlex_ime_ffi SHARED IMPORTED GLOBAL)
  set_target_properties(radishlex_ime_ffi PROPERTIES
    IMPORTED_LOCATION "${RADISHLEX_IME_FFI_LIBRARY}"
  )

  add_library(radishlex SHARED src/fcitx_addon.cpp)
  target_include_directories(radishlex PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
  )
  target_compile_definitions(radishlex PRIVATE
    RADISHLEX_RIME_SCHEMA="${RADISHLEX_RIME_SCHEMA}"
  )
  target_link_libraries(radishlex PRIVATE
    Fcitx5::Core
    radishlex_linux_platform
    radishlex_ime_ffi
  )
  set_target_properties(radishlex PROPERTIES
    PREFIX ""
    BUILD_WITH_INSTALL_RPATH ON
    INSTALL_RPATH "$ORIGIN"
    INSTALL_RPATH_USE_LINK_PATH OFF
  )
  add_custom_command(TARGET radishlex POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${RADISHLEX_IME_FFI_LIBRARY}"
            "$<TARGET_FILE_DIR:radishlex>/libradishlex_ime_ffi.so"
    COMMAND "${CMAKE_COMMAND}" -E make_directory
            "$<TARGET_FILE_DIR:radishlex>/radishlex-rime"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${RADISHLEX_RIME_DATA_SOURCE_DIR}/default.yaml"
            "${RADISHLEX_RIME_DATA_SOURCE_DIR}/radishlex_pinyin.schema.yaml"
            "${RADISHLEX_RIME_DATA_SOURCE_DIR}/pinyin_simp.dict.yaml"
            "$<TARGET_FILE_DIR:radishlex>/radishlex-rime"
    VERBATIM
  )

  configure_file(config/radishlex-addon.conf.in radishlex-addon.conf)
  configure_file(config/radishlex.conf.in radishlex.conf COPYONLY)
  install(TARGETS radishlex DESTINATION "${FCITX_INSTALL_LIBDIR}/fcitx5")
  install(FILES "${RADISHLEX_IME_FFI_LIBRARY}"
          DESTINATION "${FCITX_INSTALL_LIBDIR}/fcitx5")
  foreach(asset IN LISTS RADISHLEX_RIME_ASSETS)
    install(FILES "${RADISHLEX_RIME_DATA_SOURCE_DIR}/${asset}"
            DESTINATION "${FCITX_INSTALL_LIBDIR}/fcitx5/radishlex-rime")
  endforeach()
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/radishlex-addon.conf"
          RENAME radishlex.conf
          DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon")
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/radishlex.conf"
          DESTINATION "${FCITX_INSTALL_PKGDATADIR}/inputmethod")
endif()

if(RADISHLEX_BUILD_CONTRACT_TESTS)
  enable_testing()
  add_executable(radishlex_application_context_contract
    tests/application_context_test.cpp
    src/application_context.cpp
  )
  target_include_directories(radishlex_application_context_contract PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${RADISHLEX_IME_FFI_INCLUDE_DIR}"
  )
  target_compile_definitions(radishlex_application_context_contract PRIVATE
    RADISHLEX_APPLICATION_CONTEXT_TESTING=1
  )
  target_compile_options(radishlex_application_context_contract PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  add_test(NAME application_context
           COMMAND radishlex_application_context_contract)

  add_executable(radishlex_ffi_projection_contract
    tests/ffi_projection_test.cpp
    src/ffi_projection.cpp
    src/key_projection.cpp
  )
  target_include_directories(radishlex_ffi_projection_contract PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${RADISHLEX_IME_FFI_INCLUDE_DIR}"
  )
  target_compile_options(radishlex_ffi_projection_contract PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  add_test(NAME ffi_projection COMMAND radishlex_ffi_projection_contract)

  add_executable(radishlex_xdg_paths_contract
    tests/xdg_paths_test.cpp
    src/xdg_paths.cpp
  )
  target_include_directories(radishlex_xdg_paths_contract PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
  )
  target_compile_definitions(radishlex_xdg_paths_contract PRIVATE
    RADISHLEX_XDG_TESTING=1
  )
  target_compile_options(radishlex_xdg_paths_contract PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  add_test(NAME xdg_paths COMMAND radishlex_xdg_paths_contract)

  add_executable(radishlex_manager_runtime_contract
    tests/manager_runtime_test.cpp
    src/manager_runtime.cpp
    src/privacy_mode.cpp
    src/xdg_paths.cpp
  )
  target_include_directories(radishlex_manager_runtime_contract PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
  )
  target_compile_definitions(radishlex_manager_runtime_contract PRIVATE
    RADISHLEX_MANAGER_RUNTIME_TESTING=1
    RADISHLEX_XDG_TESTING=1
  )
  target_compile_options(radishlex_manager_runtime_contract PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  add_test(NAME manager_runtime COMMAND radishlex_manager_runtime_contract)

  if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    add_executable(radishlex_privacy_monitor_contract
      tests/privacy_monitor_test.cpp
      src/privacy_monitor.cpp
      src/privacy_mode.cpp
      src/xdg_paths.cpp
    )
    target_include_directories(radishlex_privacy_monitor_contract PRIVATE
      "${CMAKE_CURRENT_SOURCE_DIR}/include"
    )
    target_compile_definitions(radishlex_privacy_monitor_contract PRIVATE
      RADISHLEX_XDG_TESTING=1
    )
    target_compile_options(radishlex_privacy_monitor_contract PRIVATE
      -Wall -Wextra -Wpedantic -Werror
    )
    add_test(NAME privacy_monitor COMMAND radishlex_privacy_monitor_contract)
  endif()

  add_executable(radishlex_runtime_layout_contract
    tests/runtime_layout_test.cpp
    src/runtime_layout.cpp
  )
  target_include_directories(radishlex_runtime_layout_contract PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
  )
  target_compile_options(radishlex_runtime_layout_contract PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  if(CMAKE_DL_LIBS)
    target_link_libraries(radishlex_runtime_layout_contract PRIVATE
      "${CMAKE_DL_LIBS}"
    )
  endif()
  add_test(NAME runtime_layout COMMAND radishlex_runtime_layout_contract)

  add_executable(radishlex_runtime_probe
    tools/runtime_probe.cpp
    src/runtime_layout.cpp
  )
  target_include_directories(radishlex_runtime_probe PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
  )
  target_compile_options(radishlex_runtime_probe PRIVATE
    -Wall -Wextra -Wpedantic -Werror
  )
  if(CMAKE_DL_LIBS)
    target_link_libraries(radishlex_runtime_probe PRIVATE
      "${CMAKE_DL_LIBS}"
    )
  endif()
endif()
