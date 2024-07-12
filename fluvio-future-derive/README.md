# Friendly, test runner for async function

To run a test, annotate as below:
```
#[test_async]
async fn test_sum() -> Result<(), std::io::Error> {
    assert(true,"I am alive);
    Ok(())
}
```
