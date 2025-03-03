import inspect
from gmpy2 import *
import rsa
import time
from gmpy2 import gmpy2
import cachetools
import functools
import importlib.util
import sys
import powmod

@functools.cache
def mid_calc_2(b):
    x = gmpy2.isqrt(b)
    y = b-gmpy2.square(x)
    return (x,y)

def powmod_plus(a, b, modulus):
    if b > 2**40:
        x = gmpy2.isqrt(b)
        y = b-gmpy2.square(x)
        if x > y:
            t = powmod_plus(a,y,modulus)
            return gmpy2.f_mod(powmod_plus(t*powmod_plus(a,x-y,modulus),x,modulus)*t, modulus)
        else:
            t = powmod_plus(a,x,modulus)
            return gmpy2.f_mod(powmod_plus(t,x,modulus)*powmod_plus(a,y-x, modulus)*t, modulus)
    else:
        return gmpy2.powmod(a, b, modulus)

@functools.cache
def mid_calc(b):
    x = gmpy2.isqrt(gmpy2.isqrt(b))
    b_cut = b-gmpy2.square(gmpy2.square(x))
    y = gmpy2.isqrt(b_cut)
    z = b_cut-gmpy2.square(y)
    return (x,y,z)

def powmod_p(a, b, modulus):
    # print(gmpy2.bit_length(b))
    
    if b > 2**40:
        (x,y,z) = mid_calc(b)
        return gmpy2.f_mod(rep_powmod_p(4, a, x, modulus)*rep_powmod_p(2, a, y, modulus)*powmod_p(a, z, modulus), modulus)
    else:
        return gmpy2.powmod(a, b, modulus)
    
@cachetools.cached(cachetools.MRUCache(maxsize=1024))
def rep_powmod_p(reps: int, a, b, modulus):
    ret = a
    for i in range(reps):
        ret = powmod_p(ret, b, modulus)
    return ret

def wind_encoder(root, target, modulus):
            result = root
            total = 0
            for section in range(15, -1, -1):
                mask = (1 << (section * 4)) * 15 
                times = (mask & target) >> (section * 4)
                for i in range(times):
                    result = gmpy2.powmod(5, result, modulus)
                total += times
            print(total)
            return result

a = 2**64-1
b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q


#time_s = time.time()
#print(gmpy2.powmod(a, b.e, modulus))
#for i in range(0, 240000):
#    result = gmpy2.powmod(a-i, b.e, modulus)
#te = time.time() - time_s
#print(te)

print(powmod_p(37, 1202, 731231))

print(powmod_p(a, b.e, modulus))
time_s2 = time.time()
x=2**1024-1123
for i in range(1000):
    x=powmod_plus(a+i, x, modulus)
print(time.time() - time_s2)

print(gmpy2.powmod(a, b.e, modulus))
time_s3 = time.time()
for i in range(1000):
    x = gmpy2.powmod(a+i, x, modulus)
print(time.time() - time_s3)








