"""
Author: Samuel Lipsutz
Date: 2025
"""

import argparse
import json
from pathlib import Path
import hashlib
from sympy import isprime

from loguru import logger


def get_primes_starting_with(start, amount): 
    primes = []
    i = start
    while len(primes) < amount:
        i += 2
        if isprime(i):
            primes.append(i)
    return primes

def wind_encoder(root, target, exponents, modulus):
    result = root
    for bit in range(63, -1, -1):
        if (1 << bit) & target > 0:
            result = pow(exponents[bit], result, modulus)
    return result


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

def get_intermediates_hashed(start, end, root, exponents, modulus, device_hash: bytes):
    intermediates = get_intermediates(start, end, root, exponents, modulus)
    for i in intermediates:
        intermediates[i] = (intermediates[i] ^ int.from_bytes(device_hash, byteorder='big')) % modulus
    return intermediates

def pack_intermediates(intermediates: dict):
    res = b""
    positions = sorted(intermediates.keys())
    for position in positions:
        res += intermediates[position].to_bytes(128, byteorder="big")
    # Pack the remainder of the 8192 bytes
    for _ in range(8192 - len(positions) * 128):
        res += b"\x00"
    return res

def pack_inter_positions(intermediates: dict):
    res = b""
    positions = sorted(intermediates.keys())
    for position in positions:
        res += position.to_bytes(8, byteorder="big")
    # Pack the remainder of the 512 bytes
    for _ in range(512 - len(positions) * 8):
        res += b"\x00"
    return res

def pack_metadata(channel: int, modulus: int, start: int, end: int, forward_inters: dict, backward_inters: dict):
    res = len(forward_inters).to_bytes(1, byteorder='big') + len(backward_inters).to_bytes(1, byteorder='big') + \
    	channel.to_bytes(4, byteorder='big') + \
        pack_inter_positions(forward_inters) + pack_inter_positions(backward_inters) + \
        modulus.to_bytes(128, byteorder='big') + \
        start.to_bytes(8, byteorder='big') + end.to_bytes(8, byteorder='big')
    
    for _ in range(8192 - len(res)):
        res += b"\x00"
    return res

def gen_subscription(
    secrets: bytes, device_id: int, start: int, end: int, channel: int
) -> bytes:
    """Generate the contents of a subscription.

    The output of this will be passed to the Decoder using ectf25.tv.subscribe

    :param secrets: Contents of the secrets file generated by ectf25_design.gen_secrets
    :param device_id: Device ID of the Decoder
    :param start: First timestamp the subscription is valid for
    :param end: Last timestamp the subscription is valid for
    :param channel: Channel to enable
    """
    secrets = json.loads(secrets)

    modulus = secrets[str(channel)]["modulus"]
    exponents = get_primes_starting_with(1025, 16)

    forward = secrets[str(channel)]["forward"]
    backward = secrets[str(channel)]["backward"]

    end_of_time = 2**64 - 1
    forward_inters = get_intermediates(start, end, forward, exponents, modulus)
    backward_inters = get_intermediates_hashed(end_of_time - end, end_of_time - start, backward, exponents, modulus, hashlib.sha3_512(device_id.to_bytes(4)).digest())
    # Finally, we pack this like follows:
    
    # Pack the subscription. This will be sent to the decoder with ectf25.tv.subscribe
    return pack_metadata(channel, modulus, start, end, forward_inters, backward_inters) + \
        pack_intermediates(forward_inters) + pack_intermediates(backward_inters)

def parse_args():
    """Define and parse the command line arguments

    NOTE: Your design must not change this function
    """
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Force creation of subscription file, overwriting existing file",
    )
    parser.add_argument(
        "secrets_file",
        type=argparse.FileType("rb"),
        help="Path to the secrets file created by ectf25_design.gen_secrets",
    )
    parser.add_argument("subscription_file", type=Path, help="Subscription output")
    parser.add_argument(
        "device_id", type=lambda x: int(x, 0), help="Device ID of the update recipient."
    )
    parser.add_argument(
        "start", type=lambda x: int(x, 0), help="Subscription start timestamp"
    )
    parser.add_argument("end", type=int, help="Subscription end timestamp")
    parser.add_argument("channel", type=int, help="Channel to subscribe to")
    return parser.parse_args()


def main():
    """Main function of gen_subscription

    You will likely not have to change this function
    """
    # Parse the command line arguments
    args = parse_args()

    subscription = gen_subscription(
        args.secrets_file.read(), args.device_id, args.start, args.end, args.channel
    )

    # Print the generated subscription for your own debugging
    # Attackers will NOT have access to the output of this (although they may have
    # subscriptions in certain scenarios), but feel free to remove
    #
    # NOTE: Printing sensitive data is generally not good security practice
    logger.debug(f"Generated subscription: {subscription}")

    # Open the file, erroring if the file exists unless the --force arg is provided
    with open(args.subscription_file, "wb" if args.force else "xb") as f:
        f.write(subscription)

    # For your own debugging. Feel free to remove
    logger.success(f"Wrote subscription to {str(args.subscription_file.absolute())}")
    print(len(subscription))


if __name__ == "__main__":
    main()
