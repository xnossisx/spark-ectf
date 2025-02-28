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

exponents = get_primes_starting_with(1025, 16)

forward_root = rsa.randnum.read_random_int(1024)

nums = [rsa.randnum.read_random_int(64), rsa.randnum.read_random_int(64), rsa.randnum.read_random_int(64)]
nums = sorted(nums)
target = nums[1]
start = nums[0]
end = nums[2]

def wind_encoder(root, target, exponents, modulus):
    result = root
    for section in range(15, -1, -1):
        mask = (1 << (section * 4)) * 15 
        times = (mask & target) >> (section * 4)
        for i in range(times):
            result = gmpy2.powmod(exponents[section], result, modulus)
    return result

forward_end_encoder = wind_encoder(forward_root, target, exponents, modulus)


def next_required_intermediate(start):
    complement = 0
    for section in range(15, -1, -1):
        # Round up the range
        bit = (1 << (section * 4))
        mask = bit * 15 
        common = mask & start
        if common != 0:
            complement = mask - common + bit
    return start + complement


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
    for section in range(15, -1, -1):
        mask = (1 << (section * 4)) * 15 
        distance = ((mask & target) - (mask & closest)) >> (section * 4)
        for i in range(distance):
            result = gmpy2.powmod(exponents[section], result, modulus)
    return result

ts = time.time()
forward_end_decoder = wind_decoder(target, exponents, modulus, inters)
forward_end_decoder = wind_decoder(2 ** 64 - target - 1, exponents, modulus, inters)
te = time.time() - ts
print("forward_end_encoder: ", forward_end_encoder)
print("forward_end_decoder: ", forward_end_decoder)
print("forward_end_encoder == forward_end_decoder: ", forward_end_encoder == forward_end_decoder)
print("Time taken: ", te)
    