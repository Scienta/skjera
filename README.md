# Use cases

* Show my news feed
* List all employees
* Employee overview
 * Contact info
 * Current assignment
* Create customer

# Domain model

* Customer
* Employee
 * business email
 * personal email
 * phone number
 * Scienta slack name, NoA slack name
 * Other connections, linkedin, github, gitlab, X, etc
* Assignment
 * Customer
 * Employee
 * Description
 * Start/end date
* News item - generic item to be shows in someones feed
 * Kinds: new employee, leaving employee, new assignment, new company
 * Publish date, title and body. Generated from the related entities. GPT?
 * Generic relations to, Assignment, Company and Employee
