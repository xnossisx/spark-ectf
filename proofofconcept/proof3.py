import rsa
import rsa.randnum
from sympy import isprime
import time
import gmpy2
(public, private) = rsa.newkeys(1024)

modulus = public.n

def get_primes_starting_with(start, amount): 
    primes = []
    i = start
    while len(primes) < amount:
        i += 2
        if isprime(i):
            primes.append(i)
    return primes

exponents = get_primes_starting_with(1025, 64)

forward_root = rsa.randnum.read_random_int(1024)

target = 0b0011101101101010101100000001000000010011000001110110001011000011

start =  0b0001011111011000001111010000011000000010011010011011101110110000
end =    0b1110010011110011101110000011101010110111011100100111111111000000

def wind_encoder(root, target, exponents, modulus):
    result = root
    for bit in range(63, -1, -1):
        if (1 << bit) & target > 0:
            result = pow(exponents[bit], result, modulus)
    return result

forward_end_encoder = wind_encoder(forward_root, target, exponents, modulus)

def next_required_intermediate(start):
    last = 64
    for bit in range(63, -1, -1):
        if (1 << bit) & start > 0:
            last = bit
    return start + (1 << last)


def get_intermediates(start, end, root, exponents, modulus):
    intermediates = {}
    while True:
        intermediates[start] = wind_encoder(root, start, exponents, modulus)
        start = next_required_intermediate(start)
        if start > end:
            break
    return intermediates

inters = get_intermediates(start, end, forward_root, exponents, modulus)

def wind_decoder(target, exponents, modulus, intermediates: dict):
    # First, we need to get the intermediate that is closest to the target (from below).
    closest = 0
    closest_intermediate = 0

    for position in intermediates.keys():
        if position > target:
            break
        if position > closest:
            closest = position
            closest_intermediate = intermediates[position]

    # Now we need to take this intermediate to the power of each of the remaining exponents for each bit that is on in target that is not on in closest.
    result = closest_intermediate
    for bit in range(63, -1, -1):
        if (1 << bit) & target > 0 and (1 << bit) & closest == 0:
            result = gmpy2.powmod(exponents[bit], result, modulus)
    return result

ts = time.time()
forward_end_decoder = wind_decoder(target, exponents, modulus, inters)
te = time.time() - ts
print("forward_end_encoder: ", forward_end_encoder)
print("forward_end_decoder: ", forward_end_decoder)
print("forward_end_encoder == forward_end_decoder: ", forward_end_encoder == forward_end_decoder)
print("Time taken: ", te)
    