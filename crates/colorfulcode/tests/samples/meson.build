project('colorful', 'c',
  version : '1.2.0',
  license : 'MIT',
  default_options : ['warning_level=3', 'c_std=c11'])

# Dependencies
cc = meson.get_compiler('c')
m_dep = cc.find_library('m', required : false)
threads = dependency('threads')

sources = files(
  'src/main.c',
  'src/lexer.c',
)

conf = configuration_data()
conf.set_quoted('VERSION', meson.project_version())
conf.set10('HAVE_SIMD', host_machine.cpu_family() == 'x86_64')
configure_file(output : 'config.h', configuration : conf)

if get_option('buildtype').startswith('debug')
  add_project_arguments('-DDEBUG=1', language : 'c')
elif not cc.has_header('stdatomic.h')
  error('C11 atomics are required')
endif

exe = executable('colorful', sources,
  dependencies : [m_dep, threads],
  install : true)

foreach name : ['basic', 'unicode', 'large']
  test('test-' + name, exe, args : ['--test', name])
endforeach

message(f'Configured @0@ version @version@'.format(meson.project_name()))
summary({'tests' : 3, 'simd' : true}, section : 'Features')
long = '''multi
line string'''
