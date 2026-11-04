# language: en
@checkout @smoke
Feature: Shopping cart checkout
  As a customer
  I want to pay for the items in my cart
  So that they are shipped to me

  Background:
    Given the store has the following products:
      | name     | price | stock |
      | Notebook | 4.50  | 10    |
      | Pen      | 1.25  | 200   |

  Rule: Only logged in users can check out

    @happy-path
    Scenario: Paying with a valid card
      Given I am logged in as "alice@example.com"
      And I have 3 "Notebook" items in my cart
      When I pay with card "4242 4242 4242 4242"
      Then the order total should be 13.50
      But I should not be charged twice
      * a confirmation email is sent

    Scenario Outline: Applying discount codes
      Given I have <count> items in my cart
      When I apply the code "<code>"
      Then the discount is <discount> percent

      Examples:
        | count | code    | discount |
        | 1     | SAVE10  | 10       |
        | 5     | BULK25  | 25       |

  Scenario: Reading the receipt
    When I open the receipt
    Then I should see:
      """json
      { "status": "paid", "items": 3 }
      """
