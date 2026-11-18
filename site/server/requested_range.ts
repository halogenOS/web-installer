// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import parseRange from "range-parser";
import { RANGE_UNIT, type ByteRange } from "../src/remote/byte_range.ts";

// RFC 9110, section 14.2, lets a server ignore a Range and send the whole file. The route does that for every
// header other than one satisfiable range of bytes.
export function readRequestedRange(header: string | undefined, size: number): ByteRange | undefined {
  if (header === undefined) return undefined;
  const ranges = parseRange(size, header, { combine: true });
  if (!Array.isArray(ranges) || ranges.type !== RANGE_UNIT || ranges.length !== 1) return undefined;
  const [range] = ranges;
  if (range === undefined) return undefined;
  return { offset: range.start, length: range.end - range.start + 1 };
}
