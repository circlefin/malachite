# Standard Error Format

Malachite services should return errors in a consistent JSON structure.

## Structure

```json
{
  "error": {
    "code": "string",
    "message": "string",
    "details": "optional"
  }
}
