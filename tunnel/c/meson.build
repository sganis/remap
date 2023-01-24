project('tunnel_c', 'c')

ssh2_lib = 'libssh2'
ssh2_lib_dir = 'C:/Dev/golddrive/vendor/libssh2/lib/x64'
ssh2_header_dir = 'C:/Dev/golddrive/vendor/libssh2/include'

openssl_lib = 'libcrypto'
openssl_lib_dir = 'C:/Dev/golddrive/vendor/openssl/lib/x64'
openssl_header_dir = 'C:/Dev/golddrive/vendor/openssl/include'


ssh2_dep = declare_dependency(
    link_args : ['-L' + ssh2_lib_dir, '-l' + ssh2_lib],
    include_directories : include_directories(ssh2_header_dir))

openssl_dep = declare_dependency(
    link_args : ['-L' + openssl_lib_dir, '-l' + openssl_lib],
    include_directories : include_directories(openssl_header_dir))

compiler = meson.get_compiler('c')
dep = compiler.find_library('ws2_32', required: true)

executable('tunnel_c', 'tunnel_c.c', 
    dependencies: [ssh2_dep, openssl_dep, dep])
