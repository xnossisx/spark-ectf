import inspect
from gmpy2 import *
import rsa
import time

from sympy import isprime
from gmpy2 import gmpy2
from gmpy2.gmpy2 import mpz
import json

@functools.cache
def mid_calc_2(b):
    x = gmpy2.isqrt(b)
    y = b-gmpy2.square(x)
    return (x,y)

def powmod(a, b, modulus, prime_1, prime_2):
    b_red_1 = gmpy2.f_mod(b, prime_1 - 1)
    b_red_2 = gmpy2.f_mod(b, prime_2 - 1)
    q_inv = gmpy2.invert(prime_2, prime_1)
    m_1 = powmod_plus(a, b_red_1, prime_1)
    m_2 = powmod_plus(a, b_red_2, prime_2)
    sub = gmpy2.f_mod(q_inv*(m_1-m_2), mpz(prime_1))

    return gmpy2.f_mod(m_2 + (sub * prime_2), modulus)

def powmod_plus(a, b, modulus, sqrt_cache=[],depth=0):
    if gmpy2.bit_length(b) > 8:
        if len(sqrt_cache) < depth + 1:
            (x, y) = gmpy2.isqrt_rem(b)
            sqrt_cache.append((x,y))
        else:
            x,y = sqrt_cache[depth]
        return gmpy2.f_mod(powmod_plus(powmod_plus(a,x,modulus, sqrt_cache, depth+1),x,modulus, sqrt_cache, depth+1)*powmod_plus(a,y,modulus, [], 0), modulus)
    else:
        return gmpy2.powmod(a, b, modulus)

a = 1031
b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q

def wind_encoder(root, target, exponents, modulus):
    result = root
    for section in range(15, -1, -1):
        mask = (1 << (section * 4)) * 15 
        times = (mask & target) >> (section * 4)
        for i in range(times):
            result = powmod_plus(exponents[section], result, modulus)
    return result

#time_s = time.time()
#print(gmpy2.powmod(a, b.e, modulus))
#for i in range(0, 240000):
#    result = gmpy2.powmod(a-i, b.e, modulus)
#te = time.time() - time_s
#print(te)

def wind_encoder_gmp(root, target, exponents, modulus):
    result = root
    for section in range(15, -1, -1):
        mask = (1 << (section * 4)) * 15 
        times = (mask & target) >> (section * 4)
        for i in range(times):
            result = gmpy2.powmod(exponents[section], result, modulus)
    return result


#secrets = json.loads(open("/home/bruberu/ps/MITREeCTF/spark-ectf/secrets/secrets.json", "rb").read())
#modulus = secrets["0"]["modulus"]

time_s = time.time()
result = gmpy2.powmod(a, 3 ** 600, modulus)
te = time.time() - time_s
print(te)

time_s2 = time.time()
for i in range(0,1000):
    x = powmod_plus(a, 2 ** 1024 - 2 ** 1022 - i, modulus)
te_2 = time.time() - time_s2
print("powmod plus 1000 test", te_2)


print()
time_s3 = time.time()
for i in range(0,1000):
    x = gmpy2.powmod(a, 2 ** 1024 - 2 ** 1022 - i, modulus)
    #if i == 0:
        # print(x)
te_3 = time.time() - time_s3
print("gmpy 1000 test ",te_3)
print()

def get_primes_starting_with(start, amount): 
    primes = []
    i = start
    while len(primes) < amount:
        i += 2
        if isprime(i):
            primes.append(i)
    return primes

exponents = get_primes_starting_with(1025, 64)

time_s4 = time.time()
wind_encoder_gmp(14, 0xffffff, exponents, modulus)
te_4 = time.time() - time_s4
print("encoder test gmp", te_4)

time_s5 = time.time()
wind_encoder(2 ** 1024 - 1, 0xffffff, exponents, modulus)
te_5 = time.time() - time_s5
print("encoder test power plus", te_5)