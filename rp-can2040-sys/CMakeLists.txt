cmake_minimum_required(VERSION 3.13)

set(PICO_BOARD pico CACHE STRING "Target board")
set(PICO_SDK_PATH $ENV{PICO_SDK_PATH} CACHE PATH "Path to the Pico SDK")

# Must be included before project() so that the pico-sdk can set up the
# ARM cross-compilation toolchain prior to the compiler detection step.
include(${PICO_SDK_PATH}/external/pico_sdk_import.cmake)

project(can2040_lib C ASM)

pico_sdk_init()

add_library(can2040 STATIC can2040/src/can2040.c)
target_include_directories(can2040 PUBLIC can2040/src)

# Link against the specific hardware libraries can2040.c includes.
# These are interface (header-only) targets in pico-sdk, so nothing extra
# gets compiled into our static library.
target_link_libraries(can2040 PRIVATE
    hardware_dma
    hardware_pio
    hardware_gpio
    hardware_resets
)

install(TARGETS can2040 ARCHIVE DESTINATION lib)
