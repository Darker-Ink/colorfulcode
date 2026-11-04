/**
 * Keeps account ratings in sync with open opportunities.
 */
public with sharing class AccountRatingService {
    private static final Integer HOT_THRESHOLD = 5;

    @AuraEnabled(cacheable=true)
    public static List<Account> getHotAccounts(Integer minDeals) {
        return [
            SELECT Id, Name, Rating
            FROM Account
            WHERE Id IN (SELECT AccountId FROM Opportunity WHERE IsClosed = false)
            AND NumberOfEmployees > :minDeals
            ORDER BY Name ASC
            LIMIT 50
        ];
    }

    public static void updateRatings(Set<Id> accountIds) {
        Map<Id, Integer> counts = new Map<Id, Integer>();
        for (AggregateResult ar : [SELECT AccountId, COUNT(Id) total FROM Opportunity
                                   WHERE AccountId IN :accountIds GROUP BY AccountId]) {
            counts.put((Id) ar.get('AccountId'), (Integer) ar.get('total'));
        }
        List<Account> toUpdate = new List<Account>();
        for (Id accId : accountIds) {
            Integer n = counts.containsKey(accId) ? counts.get(accId) : 0;
            String rating = n >= HOT_THRESHOLD ? 'Hot' : 'Cold';
            toUpdate.add(new Account(Id = accId, Rating = rating));
        }
        try {
            update toUpdate;
        } catch (DmlException e) {
            System.debug(LoggingLevel.ERROR, 'Update failed: ' + e.getMessage() + '\n');
        }
    }

    @IsTest
    static void testRatings() {
        Account a = new Account(Name = 'Acme');
        insert a;
        Test.startTest();
        updateRatings(new Set<Id>{ a.Id });
        Test.stopTest();
        System.assertEquals('Cold', [SELECT Rating FROM Account WHERE Id = :a.Id].Rating);
    }
}
