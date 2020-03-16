# Friendly, test runner for async function

To run a test, annotate as below:
```
#[test_async]
async fn test_sum() -> Result<(),std::io::Result> {
    assert(true,"I am alive);
    Ok(())
}
```