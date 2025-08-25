use std::simd::{cmp::{SimdPartialEq, SimdPartialOrd}, f64x4, num::{SimdFloat, SimdUint}, u64x4, Simd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderDirection {
    Buy,  // prices are in descending order
    Sell,  // prices are in ascending order
}

#[derive(Clone, Copy, Debug)]
pub struct PriceInfo{
    price_data: u64  // USD price with 4 decimal places
}


#[derive(Clone, Copy, Debug)]
pub struct PriceVolInfo{
    total_price: f64,
    total_volume: f64,
}

impl PriceInfo{
    pub fn get_price_usd(&self) -> f64 {
        return (self.price_data as f64) * 1e-4;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OrderInfo{
    price: PriceInfo,
    volume: f64,
}

impl OrderInfo{
    pub fn new(price: u64, volume: f64) -> OrderInfo {
        return OrderInfo{
            price: PriceInfo{price_data: price},
            volume
        };
    }
}


#[derive(Debug)]
pub struct OrderInfo4 {
    prices_4: u64x4,
    volumes_4: f64x4,
}

impl OrderInfo4{
    pub fn new(
        start_order: &OrderInfo, 
        price_dist: u64,
    ) -> Self {
        // Can calc the required index here!
        let p_idx: u64 = start_order.price.price_data.clone() / price_dist;
        let p_big_idx: u64 = (p_idx) / 4;
        let p_idx_in_p_big: usize = (p_idx - p_big_idx * 4) as usize;
        let start_price = p_big_idx * price_dist * 4;
        let prices: [u64; 4] = [
            start_price + 0*price_dist,       
            start_price + 1*price_dist,       
            start_price + 2*price_dist,       
            start_price + 3*price_dist,       
        ];
        let mut volumes: [f64; 4] = [
            0.0f64,
            0.0f64, 
            0.0f64, 
            0.0f64, 
        ];
        volumes[p_idx_in_p_big] = start_order.volume;
        let output_data: OrderInfo4 = Self {
            prices_4: u64x4::from_array(prices),
            volumes_4: f64x4::from_array(volumes),
        };
        return output_data;
    }

    pub fn contains_price(&self, new_price: PriceInfo) -> bool {
        let pvec = u64x4::splat(new_price.price_data.clone());
        let pvec_eq = self.prices_4.simd_eq(pvec);
        let contains =  pvec_eq.any();
        return contains;
    }

    pub fn update(&mut self, new_data: OrderInfo) -> bool {
        let new_price: PriceInfo = new_data.price.clone();
        let new_vol: f64 = new_data.volume.clone();
        if self.contains_price(new_price) {
            // contains price exact. Update the exact price. No further update required
            let new_price = u64x4::splat(new_price.price_data);         
            let new_vol = f64x4::splat(new_vol);
            let price_mask = self.prices_4.simd_eq(new_price);
            self.volumes_4 = price_mask.select(new_vol, self.volumes_4);
            return true; // update has succeeded
        }  
        return false; // no yet updated
    }

    pub fn is_empty(&self) -> bool {
        let local_is_empty = self.volumes_4.reduce_sum().abs() < 1e-8;
        return local_is_empty;
    }

    pub fn get_start_price(&self) -> u64 {
        return self.prices_4.to_array()[0];
    }

    pub fn get_dollar_prices4(&self) -> f64x4 {
        let factor = f64x4::splat(1e-4 as f64);
        let prices_raw: f64x4 = self.prices_4.cast();
        return prices_raw * factor;
    }

    pub fn volume_cumsum(&self) -> f64x4 {
        // calc the cumsum volume available in this vec
        let mut cumsum_vol: f64x4 = self.volumes_4.clone();
        cumsum_vol = cumsum_vol + cumsum_vol.shift_elements_right::<1>(0.0f64);
        cumsum_vol = cumsum_vol + cumsum_vol.shift_elements_right::<2>(0.0f64);
        return cumsum_vol;
    }
    
    pub fn get_price_at_vol(&self, req_volume: f64) -> PriceVolInfo {
        // Get the price and available volume used, given a requested volume
        let cum_volume = self.volume_cumsum().shift_elements_right::<1>(0.0f64);
        let mut rem_volume = f64x4::splat(req_volume) - cum_volume;
        rem_volume = rem_volume.simd_max(f64x4::splat(0.0f64));
        let vol_used = self.volumes_4.simd_min(rem_volume);
        let total_price = (vol_used * self.get_dollar_prices4()).reduce_sum();
        let total_volume = vol_used.reduce_sum();
        let result = PriceVolInfo{total_price, total_volume};
        return result;
    }

}


#[derive(Debug)]
pub struct OrderBookSimd{
    buy_orders: Vec<OrderInfo4>,
    price_dist: u64,  // distance between two adjacent prices
}

impl OrderBookSimd{
    pub fn new(price_dist: u64) -> Self{
        return Self{
            buy_orders: Vec::new(),
            price_dist,
        };
    }

    pub fn add_buy_order(&mut self, new_buy_order: OrderInfo) {
        let mut has_updated: bool = false;
        let mut last_index: usize = 0;
        for (current_index, order_info4) in self.buy_orders.iter_mut().enumerate().rev() {
            if order_info4.update(new_buy_order.clone()) {
                has_updated = true;
                last_index = current_index;
                break;
            }
        }
        if has_updated {
            let order_at_last_idx = self.buy_orders.get(last_index);
            match order_at_last_idx {
                Some(real_last_order) => {
                    if real_last_order.is_empty() {
                        // pop the value at last index is guy is empty
                        self.buy_orders.remove(last_index);
                    }
                },
                _ => {},
            }
        } else {
            // No order found to add the new price.
            // Insert the price at given correct insert location
            self._insert_new_buy_price(new_buy_order);        
        }
    }

    pub fn get_total_volume(&self) -> f64 {
        let mut total_vol4: f64x4 = f64x4::splat(0.0f64);
        for order4 in self.buy_orders.iter() {
            total_vol4 += order4.volumes_4;
        }
        return total_vol4.reduce_sum();
    }

    fn _insert_new_buy_price(&mut self, new_buy_order: OrderInfo) {
        // Adds a new price info at the new value.
        // Linear complexity, could be improved
        let new_order4 = OrderInfo4::new(&new_buy_order, self.price_dist);
        // look for the insertion index
        let mut insert_index: usize = 0;
        for (idx, buy_order4) in self.buy_orders.iter().enumerate() {
            if buy_order4.get_start_price() < new_buy_order.price.price_data {
                insert_index = idx; 
                break;
            }
        }
        self.buy_orders.insert(insert_index, new_order4);
    }

    pub fn get_price_for_volume(&self, buy_volume: f64) -> Option<f64> {
        // Get the dollar price when buying at volume of 'buy_volume'
        let mut total_volume_filled: f64 = 0.0;
        let mut total_price_paid: f64 = 0.0;
        for order_info4 in self.buy_orders.iter().rev() {
            let price_info: PriceVolInfo = order_info4.get_price_at_vol(buy_volume - total_volume_filled);
            total_price_paid += price_info.total_price;
            total_volume_filled += price_info.total_volume;
            if total_volume_filled >= buy_volume {
                break;
            }
        }
        if total_volume_filled < buy_volume { 
            return None;
        }
        return Some(total_price_paid);
    }
}


#[derive(Debug)]
pub struct OrderBook{
    buy_orders: Vec<OrderInfo>,
}

impl OrderBook {
    pub fn new() -> Self{
        return Self{buy_orders: Vec::new()};
    }

    pub fn add_buy_order(&mut self, new_buy_order: OrderInfo) {
        let mut has_updated: bool = false;
        let mut last_index: usize = 0;
        for (current_index, order_info) in self.buy_orders.iter_mut().enumerate().rev() {
            if order_info.price.price_data == new_buy_order.price.price_data {
                // update the volume at current price
                order_info.volume = new_buy_order.volume;
                last_index = current_index;
                has_updated = true;
                break;
            }
        }
        if has_updated {
            let order_at_last_idx = self.buy_orders.get(last_index);
            match order_at_last_idx {
                Some(real_last_order) => {
                    if real_last_order.volume.abs() < 1e-8 {
                        self.buy_orders.remove(last_index);
                    }
                },
                _ => {},
            }
        } else {
            // No order found to add the new price.
            // Insert the price at given correct insert location
            self._insert_new_buy_price(new_buy_order);        
        }
    }

    fn _insert_new_buy_price(&mut self, new_buy_order: OrderInfo) {
        // Adds a new price info at the new value.
        // look for the insertion index
        let mut insert_index: usize = 0;
        for (idx, buy_order) in self.buy_orders.iter().enumerate() {
            if buy_order.price.price_data < new_buy_order.price.price_data {
                insert_index = idx; 
                break;
            }
        }
        self.buy_orders.insert(insert_index, new_buy_order.clone());
    }

    pub fn get_price_for_volume(&self, buy_volume: f64) -> Option<f64> {
        // Get the dollar price when buying at volume of 'buy_volume'
        let mut total_volume_filled: f64 = 0.0;
        let mut total_price_paid: f64 = 0.0;
        for order_info in self.buy_orders.iter().rev() {
            // Get volume we can use at the current pos
            let volume_used = (buy_volume - total_volume_filled).min(order_info.volume);
            total_volume_filled += volume_used;
            total_price_paid += volume_used * (order_info.price.get_price_usd()); 
            if (buy_volume - total_volume_filled).abs() < 1e-8 {
                break;
            }
        }
        if total_volume_filled + 1e-8 < buy_volume { 
            return None;
        }
        return Some(total_price_paid);
    }

    pub fn get_total_volume(&self) -> f64 {
        let mut total_vol: f64 = 0.0;
        for order in self.buy_orders.iter() {
            total_vol += order.volume;
        }
        return total_vol;
    }

}




#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_order_book_simd() {
        // Price every 0.1 USD.
        // Note this also matches the points we get in SuperMario.
        let price_dist = 100u64;  
        let mut order_book = OrderBookSimd::new(price_dist);
        order_book.add_buy_order(OrderInfo::new(100, 0.1));
        order_book.add_buy_order(OrderInfo::new(200, 0.2));
        order_book.add_buy_order(OrderInfo::new(300, 0.3));
        order_book.add_buy_order(OrderInfo::new(400, 0.4));
        order_book.add_buy_order(OrderInfo::new(500, 0.2));
        let price07: Option<f64> = order_book.get_price_for_volume(0.7);
        let price14: Option<f64> = order_book.get_price_for_volume(1.4);
        assert!(price07.is_some());
        assert!(price14.is_none());
        match price07 {
            Some(real_price07) => {
                assert!((real_price07-0.018).abs() < 1e-10);
            },
            None => {
                assert!(false);
            }
        }
    }

    #[test]
    fn create_order_book() {
        // Price every 0.1 USD.
        // Note this also matches the points we get in SuperMario.
        let mut order_book = OrderBook::new();
        order_book.add_buy_order(OrderInfo::new(100, 0.1));
        order_book.add_buy_order(OrderInfo::new(200, 0.2));
        order_book.add_buy_order(OrderInfo::new(300, 0.3));
        order_book.add_buy_order(OrderInfo::new(400, 0.4));
        order_book.add_buy_order(OrderInfo::new(500, 0.2));
        let price07: Option<f64> = order_book.get_price_for_volume(0.7);
        let price14: Option<f64> = order_book.get_price_for_volume(1.4);
        assert!(price07.is_some());
        assert!(price14.is_none());
        match price07 {
            Some(real_price07) => {
                assert!((real_price07-0.018).abs() < 1e-10);
            },
            None => {
                assert!(false);
            }
        }
    }

}
