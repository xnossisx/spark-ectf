"""
Author: Samuel Lipsutz
Date: 2025
"""

import json
import os
import random
import subprocess
import gen_subscription

from Crypto.PublicKey import ECC

# Get decoder ID environment variable
decoder_id = int(os.getenv("DECODER_ID"), base=0)

# Generate a seed for each channel; we need a special secret from secrets/secrets.json
secretsfile = open("/global.secrets").read()
secrets = json.loads(secretsfile)
secret = secrets["systemsecret"]
channels = secrets["channels"]

def get_key_iv(seed) -> bytes:
    val = random.Random(seed).randbytes(32)
    return val

# Generate keys the same way as the encoder does, being careful to add the emergency subscription
keys = [get_key_iv((secret << 64) + (decoder_id << 32))] + [get_key_iv((secret << 64) + (decoder_id << 32) + channel) for channel in channels]
print("Keys generated")

# Export the keys to a file
if os.path.exists("/decoder/src/keys.bin"):
    os.remove("/decoder/src/keys.bin")
open("src/keys.bin", "xb+").write(b"".join(keys))

# Generate the channel 0 subscription
sub = gen_subscription.gen_subscription(secretsfile, decoder_id, 0, 2**64 - 1, 0)
if os.path.exists("/decoder/src/emergency.bin"):
    os.remove("/decoder/src/emergency.bin")
open("/decoder/src/emergency.bin", "xb+").write(gen_subscription.gen_subscription(secretsfile, decoder_id, 0, 2**64 - 1, 0))
print("Emergency subscription generated")

# Export public ECC key into file
curve = ECC.import_key(encoded=secrets["public"], curve_name="Ed25519")
if os.path.exists("/decoder/src/public.bin"):
    os.remove("/decoder/src/public.bin")
with open("/decoder/src/public.bin", "xb+") as f:
    # Dump the secrets to the file
    f.write(curve.public_key().export_key(format='raw'))


# Set the CHANNELS env variable to the channels (other than 0) concatenated with commas
os.putenv("CHANNELS", ",".join([str(channel) for channel in channels if channel != 0]))

# Build the decoder
subprocess.run(["cargo", "build", "--profile", "release"], cwd=".")
# Convert it into the right structure and move it to /out
subprocess.run(["arm-none-eabi-objcopy", "--output-target=binary", "target/thumbv7em-none-eabihf/release/spark-decoder", "/out/max78000.bin"], cwd=".")