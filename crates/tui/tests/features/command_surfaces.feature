Feature: Eval smoke test

  Scenario: Binary loads and reports step-level success via eval
    Given a clean CodeWhale evaluation workspace
    When the evaluation harness runs a shell command
    Then the binary exits successfully
    And the JSON report contains execution steps
