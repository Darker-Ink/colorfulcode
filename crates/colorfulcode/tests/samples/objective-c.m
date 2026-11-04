#import <Foundation/Foundation.h>
#import "Account.h"

#define MAX_BALANCE 1000000

// A bank account with a running balance.
@interface Account : NSObject <NSCopying>

@property (nonatomic, copy) NSString *owner;
@property (nonatomic, assign, readonly) NSInteger balance;

- (instancetype)initWithOwner:(NSString *)owner;
- (BOOL)deposit:(NSInteger)amount error:(NSError **)error;
+ (Account *)accountWithOwner:(NSString *)owner;

@end

@implementation Account {
    NSMutableArray<NSNumber *> *_history;
}

- (instancetype)initWithOwner:(NSString *)owner {
    if ((self = [super init])) {
        _owner = [owner copy];
        _history = [[NSMutableArray alloc] init];
    }
    return self;
}

+ (Account *)accountWithOwner:(NSString *)owner {
    return [[self alloc] initWithOwner:owner];
}

- (BOOL)deposit:(NSInteger)amount error:(NSError **)error {
    if (amount <= 0 || _balance + amount > MAX_BALANCE) {
        NSLog(@"Rejected deposit of %ld\n", (long)amount);
        return NO;
    }
    _balance += amount;
    [_history addObject:@(amount)];
    return YES;
}

- (void)dealloc {
    [_history removeAllObjects];
}

@end
