"""
Author: Samuel Lipsutz
Date: 2025
"""

import json
import os
import random as rnd
import rsa.common
import rsa.prime
import struct
import typing
import rsa.transform
import subprocess

def read_random_bits(nbits: int, random: rnd.Random) -> bytes:
    """Reads 'nbits' random bits.

    If nbits isn't a whole number of bytes, an extra byte will be appended with
    only the lower bits set.
    """

    nbytes, rbits = divmod(nbits, 8)

    # Get the random bytes
    randomdata = random.randbytes(nbytes)

    # Add the remaining random bits
    if rbits > 0:
        randomvalue = ord(random.getrandbits(1))
        randomvalue >>= 8 - rbits
        randomdata = struct.pack("B", randomvalue) + randomdata

    return randomdata


def read_random_int(nbits: int, random: rnd.Random) -> int:
    """Reads a random integer of approximately nbits bits."""

    randomdata = read_random_bits(nbits, random)
    value = rsa.transform.bytes2int(randomdata)

    # Ensure that the number is large enough to just fill out the required
    # number of bits.
    value |= 1 << (nbits - 1)

    return value


def read_random_odd_int(nbits: int, random: rnd.Random) -> int:
    """Reads a random odd integer of approximately nbits bits.

    >>> read_random_odd_int(512) & 1
    1
    """

    value = read_random_int(nbits, random)

    # Make sure it's odd
    return value | 1

def getprime(random: rnd.Random):
    """Returns a prime number that can be stored in 'nbits' bits."""
    def getprimeseed(nbits: int) -> int:
    
        assert nbits > 3  # the loop will hang on too small numbers
    
        while True:
            integer = read_random_odd_int(nbits, random)
    
            # Test for primeness
            if rsa.prime.is_prime(integer):
                return integer
    
                # Retry if not prime
    return getprimeseed

def gen_keys_seed(
        nbits: int,
        seed: int,
) -> typing.Tuple:
    return rsa.key.gen_keys(nbits, getprime(rnd.Random(seed)))

# Get decoder ID environment variable
decoder_id = os.getenv("DECODER_ID")

# Generate a seed for each channel; you need a secret from secrets/secrets.json
secrets = json.loads(os.open("secrets/secrets.json", os.O_RDONLY))
secret = secrets["systemsecret"]
channels = secrets["channels"]
seeds = [gen_keys_seed(1280, (secret << 64) + (decoder_id << 32) + channel) for channel in channels]