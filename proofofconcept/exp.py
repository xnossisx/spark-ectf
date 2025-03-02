from gmpy2 import *
import rsa
import time

def powmod(a, b, modulus, prime_1, prime_2):
    b_red_1 = gmpy2.f_mod(b, prime_1 - 1)
    b_red_2 = gmpy2.f_mod(b, prime_2 - 1)
    q_inv = gmpy2.invert(prime_2, prime_1)
    m_1 = gmpy2.powmod(a, b_red_1, prime_1)
    m_2 = gmpy2.powmod(a, b_red_2, prime_2)

    sub = gmpy2.f_mod(q_inv*(m_1-m_2), mpz(prime_1))

    return gmpy2.f_mod(m_2 + (sub * prime_2), modulus)

a = 72340941231231244562348902492807365

b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q

time_s = time.time()
print (powmod(a+2,b.e,modulus,p,q))
#result = gmpy2.powmod(a, b.e, modulus)
#print (result)
te = time.time() - time_s
print(te)

b, priv = rsa.newkeys(1024)
modulus = b.n

p, q = priv.p, priv.q

time_s2 = time.time()
print (powmod(a+8,b.e,modulus,p,q))
te_2 = time.time() - time_s2
print(te_2)












