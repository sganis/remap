project('tunnel_c', 'c')


ssh2_dep = dependency('libssh2')
openssl_dep = dependency('openssl')

compiler = meson.get_compiler('c')
dep = compiler.find_library('ws2_32', required: true)

executable('tunnel_c', 'tunnel_c.c', 
    dependencies: [ssh2_dep, openssl_dep, dep],
    link_args : '/NODEFAULTLIB:MSVCRT')
