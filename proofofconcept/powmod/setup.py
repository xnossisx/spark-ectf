from setuptools import setup, Extension

# Define the extension module
module = Extension('powmod',
                   sources=['powmod.c', 'vector-master/vector.c'],
                   libraries=['gmp'])

# Setup the module
setup(name='powmod',
      version='1.0',
      description='CPython improvement on GMP powmod function',
      ext_modules=[module],
      author='XnossisX',
      author_email='xnossisx@gmail.com')