cmake_minimum_required(VERSION 3.10)

project(Testing)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_C_STANDARD 17)

# default Cargo target folder
cmake_path(SET BASE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release")

# overwrite DEPENDENCIES_DIR from command line if your target folder is somewhere else
set(DEPENDENCIES_DIR ${BASE_PATH} CACHE PATH "folder with mboot library files")

if (NOT(WIN32 OR APPLE OR UNIX))
    message(WARNING "Your operating system is not supported, configuration will proceed as on UNIX system.")
endif()

if (WIN32)
    cmake_path(APPEND DEPENDENCIES_DIR "mboot.dll" OUTPUT_VARIABLE LIBRARY_PATH)
elseif(APPLE)
    cmake_path(APPEND DEPENDENCIES_DIR "libmboot.dylib" OUTPUT_VARIABLE LIBRARY_PATH)
else()
    cmake_path(APPEND DEPENDENCIES_DIR "libmboot.so" OUTPUT_VARIABLE LIBRARY_PATH)
endif()

message("library path: ${LIBRARY_PATH}")
# find mboot library
find_library(MBOOT mboot PATHS ${DEPENDENCIES_DIR})

# get all c and c++ source files
file (GLOB SOURCE_FILES *.c *.cpp)

foreach(SOURCE_FILE ${SOURCE_FILES})
    get_filename_component(FILE_NAME ${SOURCE_FILE}, NAME_WE)
    add_executable(${FILE_NAME} ${SOURCE_FILE})
    # link mboot
    target_link_libraries(${FILE_NAME} "${MBOOT}")

    # copy file at LIRARY_PATH to the same folder as target, if it was changed or does not exist
    add_custom_command(TARGET ${FILE_NAME} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        ${LIBRARY_PATH}
        $<TARGET_FILE_DIR:${FILE_NAME}>
    )
endforeach()

