#!/usr/bin/env python3
"""presign.py put|get <key>: print a presigned URL for <key> under the campaign's
prefix in the dive evidence archive bucket. The URL is only ever passed to SSM
commands (the node uploads its raw tarball with curl -T, or downloads a binary),
so the node needs no bucket credentials.

Environment: AWS_PROFILE, AWS_REGION (default us-east-1), DIVE_EVIDENCE_BUCKET
(bucket name), DIVE_PERF_PREFIX (for example release-perf/2026-11-18/).
"""
import os
import sys

import boto3

s = boto3.Session(profile_name=os.environ["AWS_PROFILE"], region_name=os.environ.get("AWS_REGION", "us-east-1"))
bucket = os.environ["DIVE_EVIDENCE_BUCKET"]
key = os.environ["DIVE_PERF_PREFIX"] + sys.argv[2]
op = "put_object" if sys.argv[1] == "put" else "get_object"
print(s.client("s3").generate_presigned_url(op, Params={"Bucket": bucket, "Key": key}, ExpiresIn=6 * 3600))
