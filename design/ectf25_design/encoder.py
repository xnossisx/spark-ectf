"""
Author: Garrick Schinkel
Date: 2025



"""

import argparse
import struct
import json
from sympy import isprime


def extended_gcd(a, b):
    """
    Extended Euclidean Algorithm to find GCD(a, b) and coefficients x, y such that ax + by = GCD(a, b).

    Args:
      a: First integer.
      b: Second integer.

    Returns:
      A tuple (g, x, y) where g is GCD(a, b) and x, y are coefficients.
    """
    if b == 0:
        return a, 1, 0
    else:
        g, x1, y1 = extended_gcd(b, a % b)
        x = y1
        y = x1 - (a // b) * y1
        return g, x, y
    
def modular_inverse(a, m):
    """
    Calculates the modular inverse of a modulo m using the Extended Euclidean Algorithm.

    Args:
      a: The integer for which to find the inverse.
      m: The modulus.

    Returns:
      The modular inverse of a modulo m, or None if it doesn't exist.
    """
    g, x, y = extended_gcd(a, m)
    if g != 1:
        return None  # Inverse doesn't exist if a and m are not coprime
    else:
        return x % m

def fletcher32(string):
    a = list(map(ord,string))
    b = [sum(a[:i])%65535 for i in range(len(a)+1)]
    return (sum(b) << 16) | max(b)


class Encoder:

    def __init__(self, secrets: bytes):
        """
        You **may not** change the arguments or returns of this function!

        :param secrets: Contents of the secrets file generated by
            ectf25_design.gen_secrets
        """
        # TODO: parse your secrets data here and run any necessary pre-processing to
        #   improve the throughput of Encoder.encode

        def get_primes_starting_with(start, amount): 
            primes = []
            i = start
            while len(primes) < amount:
                i += 2
                if isprime(i):
                    primes.append(i)
            return primes
        self.exponents = get_primes_starting_with(1025, 64)

        # Load the json of the secrets file
        secrets = json.loads(secrets)

        # Load the example secrets for use in Encoder.encode
        # This will be "EXAMPLE" in the reference design"
        self.secrets = secrets

    def encode(self, channel: int, frame: bytes, timestamp: int) -> bytes:
        """The frame encoder function

        This will be called for every frame that needs to be encoded before being
        transmitted by the satellite to all listening TVs

        You **may not** change the arguments or returns of this function!

        :param channel: 16b unsigned channel number. Channel 0 is the emergency
            broadcast that must be decodable by all channels.
        :param frame: Frame to encode. Max frame size is 64 bytes.
        :param timestamp: 64b timestamp to use for encoding. **NOTE**: This value may
            have no relation to the current timestamp, so you should not compare it
            against the current time. The timestamp is guaranteed to strictly
            monotonically increase (always go up) with subsequent calls to encode

        :returns: The encoded frame, which will be sent to the Decoder
        """
        # TODO: encode the satellite frames so that they meet functional and
        #  security requirements
        def wind_encoder(root, target, exponents, modulus):
            result = root
            for bit in range(0, 64):
                if (1 << bit) & target > 0:
                    result = pow(result, exponents[bit], modulus)
                return result
        
        forward_root = self.secrets[channel]["forward"]
        backward_root = self.secrets[channel]["backward"]
        modulus = self.secrets[channel]["modulus"]
        end_of_time = 2**64 - 1
        forward = wind_encoder(forward_root, timestamp, self.exponents, modulus)
        backward = wind_encoder(backward_root, end_of_time - timestamp, self.exponents, modulus)

        guard = forward ^ backward
        guard_inverse = modular_inverse(guard, modulus)

        encoded = pow(int(frame), guard_inverse, modulus).to_bytes(64, byteorder="big")
        
        checksum = fletcher32(frame).to_bytes(4, byteorder="big")
        return struct.pack("<IQ", channel, timestamp) + encoded + checksum


def main():
    """A test main to one-shot encode a frame

    This function is only for your convenience and will not be used in the final design.

    After pip-installing, you should be able to call this with:
        python3 -m ectf25_design.encoder path/to/test.secrets 1 "frame to encode" 100
    """
    parser = argparse.ArgumentParser(prog="ectf25_design.encoder")
    parser.add_argument(
        "secrets_file", type=argparse.FileType("rb"), help="Path to the secrets file"
    )
    parser.add_argument("channel", type=int, help="Channel to encode for")
    parser.add_argument("frame", help="Contents of the frame")
    parser.add_argument("timestamp", type=int, help="64b timestamp to use")
    args = parser.parse_args()

    encoder = Encoder(args.secrets.read())
    print(repr(encoder.encode(args.channel, args.frame.encode(), args.timestamp)))


if __name__ == "__main__":
    main()
