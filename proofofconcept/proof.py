import rsa
import random
import time
from gmpy2 import mpz, powmod

(public, private) = rsa.newkeys(1024)

modulus = public.n
exponent = 65537
totient = (private.p - 1) * (private.q - 1)

forward_0 = random.getrandbits(1024)
backward_end = random.getrandbits(1024)

# Test subscription
end_of_time = 2 ** 64 - 1

subscription_start = 10
subscription_end = end_of_time - 39
test = 20

def stone(totient, base, location):
    res = base
    for _ in range(location):
        res *= res
        res %= totient
    return res 

def wind_totient(key, distance, exponent, modulus, totient):
    # First, we generate the exponent to the key.
    key_exp = 1
    bit_position = 0
    bit = 1
    while bit <= distance:
        if bit & distance > 0:
            key_exp *= stone(totient, exponent, bit_position)
            key_exp %= totient
        bit *= 2
        bit_position += 1
    # Then, we just return the power.
    return pow(key, key_exp, modulus)

stones = [exponent]
for l in range(4, 64, 4):
    stones.append(stone(totient, exponent, l))


f_sub = wind_totient(forward_0, subscription_start, exponent, modulus, totient)
b_sub = wind_totient(backward_end, end_of_time - subscription_end, exponent, modulus, totient)

f_test_encoder = wind_totient(forward_0, test, exponent, modulus, totient)
b_test_encoder = wind_totient(backward_end, end_of_time - test, exponent, modulus, totient)

hope_encoder = f_test_encoder ^ b_test_encoder


def wind_stones(key, distance, modulus, stones):
    # Since we don't have access to the totient, we have to use the particular stones we received one at a time.
    # Within each range of bits (0-3, 4-7, 8-11, etc), we must see if there is a bit we need to set.

    result = mpz(key)
    for stone_section in range(0, 16):
        steps = -1 # Number of steps from the initial stone; start at -1 to mean that it won't even be iterated over
        for bit in range(3, -1, -1):
            if distance & (1 << (bit + stone_section * 4)) > 0:
                steps = bit # We do care about using this stone.
                break
        # Each "stone" is just E^(2^bit) mod the totient. We need to multiply this by itself to double that exponent.
        exponent = mpz(stones[stone_section])
        
        for bit in range(stone_section * 4, stone_section * 4 + steps + 1):
            start = time.time()
            if distance & (1 << bit) > 0:
                result = powmod(result, exponent, modulus)
            exponent *= exponent
            end = time.time()
            print("Bit: ", bit)
            print("Time taken: ", end - start)
    return int(result)

start = time.time()

f_test_decoder = wind_stones(f_sub, test - subscription_start, modulus, stones)
b_test_decoder = wind_stones(b_sub, subscription_end - test, modulus, stones) 
hope_decoder = f_test_decoder ^ b_test_decoder

end = time.time()
print("Time taken: ", end - start)