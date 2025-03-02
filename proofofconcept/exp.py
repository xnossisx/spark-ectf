from gmpy2 import *
import rsa
import time

from gmpy2 import gmpy2
from gmpy2.gmpy2 import mpz


def powmod(a, b, modulus, prime_1, prime_2):
    b_red_1 = gmpy2.f_mod(b, prime_1 - 1)
    b_red_2 = gmpy2.f_mod(b, prime_2 - 1)
    q_inv = gmpy2.invert(prime_2, prime_1)
    m_1 = powmod_plus(a, b_red_1, prime_1)
    m_2 = powmod_plus(a, b_red_2, prime_2)
    sub = gmpy2.f_mod(q_inv*(m_1-m_2), mpz(prime_1))

    return gmpy2.f_mod(m_2 + (sub * prime_2), modulus)

def powmod_plus(a, b, modulus):
    if gmpy2.bit_length(b) > 48:
        x = gmpy2.isqrt(b)
        y = b-gmpy2.square(x)
        return gmpy2.f_mod(powmod_plus(powmod_plus(a,x,modulus),x,modulus)*powmod_plus(a,y,modulus))
    else:
        return gmpy2.powmod(a, b, modulus)



a = 2**64-1
b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q



time_s = time.time()
print (powmod(a+1,b.e,modulus,p,q))
#result = gmpy2.powmod(a, b.e, modulus)
#print (result)
te = time.time() - time_s
print(te)

print(powmod_plus(a+1, b.e, modulus))

b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q




print()
time_s3 = time.time()
for i in range(0,1000):
    x = powmod_plus(a - i, b.e, modulus)
    #if i == 0:
        # print(x)
te_3 = time.time() - time_s3
print(te_3)
print()
time_s2 = time.time()
print (powmod(a+8,b.e,modulus,p,q))
te_2 = time.time() - time_s2
print(te_2)











