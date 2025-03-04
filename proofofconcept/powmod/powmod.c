#include <Python.h>
#include <gmp.h>
#include "vector-master/vector.h"


// Helper function to initialize multiple mpz_t variables at once
static void init(mpz_t x, mpz_t y, mpz_t o1, mpz_t o2, mpz_t m1) {
    mpz_init(x);
    mpz_init(y);
    mpz_init(o1);
    mpz_init(o2);
    mpz_init(m1);
}

// Helper function to get bit length of mpz_t
static int mpz_bit_length(mpz_t n) {
    return mpz_sizeinbase(n, 2);
}

static void clear_destroy(Vector* v) {
    for (size_t i = 0; i < v->size; i++) {
        mpz_clear(vector_get(v, i));
    }
    vector_clear(v);
    vector_destroy(v);
}

// The actual implementation of powmod_plus
static void powmod_impl(mpz_t ret, mpz_t a, mpz_t b, mpz_t modulus, Vector* sqrts, Vector* squares, int depth) {
    if (mpz_bit_length(b) > 48) {
        mpz_t x, y, o1, o2, m1;
        init(x, y, o1, o2, m1);
        
        if (sqrts->size <= depth) {
            mpz_set(x, vector_get(sqrts, depth));
            mpz_set(y, vector_get(squares, depth));
        } else {
            mpz_sqrt(x, b);  // Changed from mpz_isqrt to mpz_sqrt
            mpz_mul(m1, x, x);
            mpz_sub(y, b, m1);
            vector_push_back(sqrts, x);
            vector_push_back(squares, y);
        }
        
        powmod_impl(o1, a, x, modulus, sqrts, squares, depth+1);
        powmod_impl(o1, o1, x, modulus, sqrts, squares, depth+1);

        Vector y_sqrts;
        Vector y_squares;

        vector_setup(&y_sqrts, 16-depth, sizeof(mpz_t));
        vector_setup(&y_squares, 16-depth, sizeof(mpz_t));

        powmod_impl(o2, a, y, modulus, &y_sqrts, &y_squares, 0);
        
        clear_destroy(&y_sqrts);
        clear_destroy(&y_squares);

        // Multiply o1 and o2, then take modulus
        mpz_mul(ret, o1, o2);
        mpz_mod(ret, ret, modulus);
        
        // Clear all temporary variables
        mpz_clear(x);
        mpz_clear(y);
        mpz_clear(o1);
        mpz_clear(o2);
        mpz_clear(m1);
    } else {
        mpz_powm(ret, a, b, modulus);
        mpz_mod(ret, ret, modulus);
    }
}


// Function to compute (base^exp) % mod
static PyMODINIT_FUNC powmod_plus(PyObject* self, PyObject* args) {
    PyObject* py_base, py_exp, py_modulus;
    if (!PyArg_ParseTuple(args, "OOO", py_base, py_exp, py_modulus)) {
        PyRun_SimpleString("print('hello')\n"); 
        return NULL;
    }

    PyRun_SimpleString("print('hello')\n"); 

    mpz_t base, exp, mod, result;
    mpz_inits(base, exp, mod, result, NULL);
    
    /* Convert Python strings to GMP integers */
    PyObject_Bytes
    PyObject* str_base = PyObject_Bytes(&py_base);
    PyObject* str_exp = PyObject_Bytes(&py_exp);
    PyObject* str_mod = PyObject_Bytes(&py_modulus);
    const char* sbase, sexp, smod;
    _PyLong_AsNativeBytes(str_base, sbase, )
    //const char* sbase = PyBytes_FromStringAndSize(NULL, py_base);
    //const char* sexp = PyObject_AsCharBuffer(str_exp);
    //const char* smod = PyObject_AsCharBuffer(str_mod);
    mpz_import(base, 256, 1, 1, 0, 0, sbase);
    mpz_import(exp, 256, 1, 1, 0, 0, sexp);
    mpz_import(mod, 256, 1, 1, 0, 0, smod);
    Py_DECREF(str_base); Py_DECREF(str_base); Py_DECREF(str_mod);
    
    PyRun_SimpleString("print(\"Hello from C!\")");

    // Call our implementation
    // Initialize this with a certain capacity
    Vector sqrts;
    vector_setup(&sqrts, 16, sizeof(mpz_t));
    Vector squares;
    vector_setup(&squares, 16, sizeof(mpz_t));
    powmod_impl(result, base, exp, mod, &sqrts, &squares, 0);
    
    // Convert result to string
    char* result_str = mpz_get_str(NULL, 10, result);
    
    // Create Python string object
    PyObject* py_result = PyUnicode_FromString(result_str);
    
    vector_clear(&sqrts);
    vector_destroy(&squares);

    // Free GMP resources
    mpz_clear(base);
    mpz_clear(exp);
    mpz_clear(mod);
    mpz_clear(result);
    free(result_str);
    
    return py_result;
}



// Method definition
static PyMethodDef PowmodMethods[] = {
    {"powmod_plus", powmod_plus, METH_VARARGS, "Compute (base^exp) % mod using GMP."},
    {NULL, NULL, 0, NULL}  // Sentinel
};

// Module definition
static struct PyModuleDef powmodmodule = {
    PyModuleDef_HEAD_INIT,
    "powmod",      // Module name
    "GMP-powered modular exponentiation with addition",  // Module docstring
    -1,                // Size of per-interpreter state or -1
    PowmodMethods      // Method table
};

// Module initialization function
PyMODINIT_FUNC PyInit_powmod(void) {
    return PyModule_Create(&powmodmodule);
}