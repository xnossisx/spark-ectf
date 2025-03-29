"""
Author: Samuel Lipsutz
Date: 2025
"""

import argparse
import json
from pathlib import Path

from loguru import logger
import rsa
from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Hash import SHA512
def gen_secrets(channels: list[int]) -> bytes:
    """Generate the contents secrets file

    This will be passed to the Encoder, ectf25_design.gen_subscription, and the build
    process of the decoder

    :param channels: List of channel numbers that will be valid in this deployment.
        Channel 0 is the emergency broadcast, which will always be valid and will
        NOT be included in this list

    :returns: Contents of the secrets file
    """
    # TODO: Update this function to generate any system-wide secrets needed by
    #   your design

    # Create the secrets object
    # You can change this to generate any secret material
    # The secrets file will never be shared with attackers
    secrets = {}

    channels.append(0)  # Add the broadcast channel
    secrets["channels"] = channels

    # For helping encrypt subscriptions
    secrets["systemsecret"] = rsa.randnum.read_random_int(64)
    # For frame verification
    curve = ECC.generate(curve='Ed25519') # Randomness included
    # Private signature key and public signature key
    secrets["private"] = curve.export_key(format='PEM', protection='PBKDF2WithHMAC-SHA512AndAES128-CBC')
    secrets["public"] = curve.public_key().export_key(format='PEM')



    for channel in channels:
        secrets[channel] = {}
        # These are just hashed, so their values don't really have any significance
        secrets[channel]["forward"] = rsa.randnum.read_random_int(128)
        secrets[channel]["backward"] = rsa.randnum.read_random_int(128)


    # NOTE: if you choose to use JSON for your file type, you will not be able to
    # store binary data, and must either use a different file type or encode the
    # binary data to hex, base64, or another type of ASCII-only encoding
    return json.dumps(secrets).encode()


def parse_args():
    """Define and parse the command line arguments

    NOTE: Your design must not change this function
    """
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Force creation of secrets file, overwriting existing file",
    )
    parser.add_argument(
        "secrets_file",
        type=Path,
        help="Path to the secrets file to be created",
    )
    parser.add_argument(
        "channels",
        nargs="+",
        type=int,
        help="Supported channels. Channel 0 (broadcast) is always valid and will not"
        " be provided in this list",
    )
    return parser.parse_args()


def main():
    """Main function of gen_secrets

    You will likely not have to change this function
    """
    # Parse the command line arguments
    args = parse_args()

    secrets = gen_secrets(args.channels)

    # Print the generated secrets for your own debugging
    # Attackers will NOT have access to the output of this, but feel free to remove
    #
    # NOTE: Printing sensitive data is generally not good security practice
    logger.debug(f"Generated secrets: {secrets}")

    # Open the file, erroring if the file exists unless the --force arg is provided
    with open(args.secrets_file, "wb" if args.force else "xb") as f:
        # Dump the secrets to the file
        f.write(secrets)

if __name__ == "__main__":
    main()
