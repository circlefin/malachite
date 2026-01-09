# Standard Error Format

This document defines the standard error format used across Malachite components.

## Error object structure

All errors should follow this structure:

{
  "error": {
    "code": "string",
    "message": "string",
    "details": "optional"
  }
}

## Fields

- code: A stable, machine-readable identifier
- message: A short, human-readable explanation
- details: Optional extra context for debugging

## Guidelines

- Do not expose secrets or internal stack traces
- Keep error messages concise and actionable
- Reuse existing error codes where possible
- Document new error codes when introduced

## Example

{
  "error": {
    "code": "INVALID_CONFIGURATION",
    "message": "Missing required network parameter",
    "details": "NETWORK_ID was not provided"
  }
}
