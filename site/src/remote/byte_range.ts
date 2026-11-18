// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

export const PARTIAL_CONTENT = 206;

export const RANGE_UNIT = "bytes";

export interface ByteRange {
  offset: number;
  length: number;
}

export function lastByteOf(range: ByteRange): number {
  return range.offset + range.length - 1;
}

export function rangeHeader(range: ByteRange): string {
  return `${RANGE_UNIT}=${String(range.offset)}-${String(lastByteOf(range))}`;
}

export function contentRangeHeader(range: ByteRange, size: number): string {
  return `${RANGE_UNIT} ${String(range.offset)}-${String(lastByteOf(range))}/${String(size)}`;
}
