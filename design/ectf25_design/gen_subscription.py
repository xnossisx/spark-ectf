"""
Author: Samuel Lipsutz
Date: 2025
"""

import argparse
import json
from pathlib import Path
import random
from loguru import logger
from blake3 import blake3
from Crypto.Cipher import AES


# Hashes it with Blake3
def compress(n, section):
    compressed = int.from_bytes(blake3(section.to_bytes(1, byteorder="big")).update(n.to_bytes(16, byteorder="big")).digest()) & (2 ** 128 - 1)
    return compressed


def wind_encoder(root, target):
    result = root
    for section in range(64, -1, -1):
        mask = 1 << section
        if mask & target > 0:
            result = compress(result, section)
    return result
            
def next_required_intermediate(start):
    complement = 0
    for section in range(64, -1, -1):
        # Round up the range
        bit = 1 << section
        if bit & start > 0:
            complement = bit
    return start + complement


def get_intermediates(start, end, root):
    intermediates = {}
    if start == 0:
        intermediates[start] = root
        return intermediates
    while True:
        intermediates[start] = wind_encoder(root, start)
        start = next_required_intermediate(start)
        if start > end:
            break
    return intermediates

def pack_intermediates(intermediates: dict, secret: int):
    _res = b""
    positions = sorted(intermediates.keys())
    for position in positions:
        val = encrypt(intermediates[position].to_bytes(16, byteorder="big"), secret)
        _res += val
    # Pack the remainder of the 1024 bytes
    for _ in range((64 * 16) - len(positions) * 16):
        _res += b"\x00"
    return _res

def pack_inter_positions(intermediates: dict):
    _res = b""
    positions = sorted(intermediates.keys())
    for position in positions:
        _res += position.to_bytes(8, byteorder="big")
    # Pack the remainder of the 512 bytes
    for _ in range(512 - len(positions) * 8):
        _res += b"\x00"
    return _res

def pack_metadata(channel: int, start: int, end: int, forward_inters: dict, backward_inters: dict):
    _res = channel.to_bytes(4, byteorder='big') + \
        start.to_bytes(8, byteorder='big') + end.to_bytes(8, byteorder='big') + \
    	len(forward_inters).to_bytes(1, byteorder='big') + len(backward_inters).to_bytes(1, byteorder='big') + \
        pack_inter_positions(forward_inters) + pack_inter_positions(backward_inters)
    
    for _ in range(1280 - len(_res)):
        _res += b"\x00"
    return _res

def encrypt(data, seed):
    key = random.Random(seed).randbytes(32)

    cipher = AES.new(key[:16], AES.MODE_OFB, iv=key[16:])

    return cipher.encrypt(data)

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

    forward = secrets[str(channel)]["forward"]
    backward = secrets[str(channel)]["backward"]

    end_of_time = 2**64 - 1
    forward_inters = get_intermediates(start, end, forward)

    backward_inters = get_intermediates(end_of_time - end, end_of_time - start, backward)
    # Finally, we pack this like follows:
    secret = (secrets["systemsecret"] << 64) + (device_id << 32) + channel

    # Pack the subscription. This will be sent to the decoder with ectf25.tv.subscribe
    return pack_metadata(channel, start, end, forward_inters, backward_inters) + \
        pack_intermediates(forward_inters, secret) + pack_intermediates(backward_inters, secret)

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


if __name__ == "__main__":
    main()
